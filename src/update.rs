//! Once-a-day check for a newer release.
//!
//! Rendering never waits on the network. It reads the tag cached by the last check
//! and, once that is a day old, hands a fresh check to a detached copy of this binary.

use crate::render::{ORANGE, Section, WHITE};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, UNIX_EPOCH};

/// Runs the check itself, in the detached process. Claude Code never passes it.
pub const CHECK_FLAG: &str = "--check-update";

const CHECK_INTERVAL_SECS: u64 = 86_400;
const CACHE_FILE: &str = "claude_statusline.update";

/// On unless `CLAUDE_STATUSLINE_UPDATE_CHECK` is off, or Claude Code's
/// `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC` is on.
pub fn enabled(var: impl Fn(&str) -> Option<String>) -> bool {
    let truthy = |value: String| {
        !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "" | "0" | "false" | "no" | "off"
        )
    };
    var("CLAUDE_STATUSLINE_UPDATE_CHECK").is_none_or(truthy)
        && !var("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC").is_some_and(truthy)
}

/// In Claude Code's config directory: `$CLAUDE_CONFIG_DIR`, else `~/.claude`.
pub fn cache_path(var: impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let non_empty = |name: &str| var(name).filter(|value| !value.is_empty());
    let dir = non_empty("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| non_empty("HOME").map(|home| Path::new(&home).join(".claude")))?;
    Some(dir.join(CACHE_FILE))
}

/// The cached latest tag, starting a background check first when the cache is due.
///
/// `now` is Unix epoch seconds. The cache file's mtime records the last check.
pub fn latest_release(path: &Path, now: u64) -> Option<String> {
    let checked_at = fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|age| age.as_secs());
    if is_due(checked_at, now) && claim(path, now).is_ok() {
        spawn_check(path);
    }
    let tag = fs::read_to_string(path).ok()?;
    Some(tag.trim().to_string()).filter(|tag| !tag.is_empty())
}

/// A missing cache is due. A clock behind the cache waits until it catches up.
pub fn is_due(checked_at: Option<u64>, now: u64) -> bool {
    checked_at.is_none_or(|at| now.saturating_sub(at) >= CHECK_INTERVAL_SECS)
}

/// Stamp the cache as checked, so the renders that follow don't start checks of their own.
/// A check that fails then waits a day too, instead of retrying on every render.
fn claim(path: &Path, now: u64) -> std::io::Result<()> {
    fs::File::options()
        .create(true)
        .append(true)
        .open(path)?
        .set_modified(UNIX_EPOCH + Duration::from_secs(now))
}

fn spawn_check(path: &Path) {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let mut command = Command::new(exe);
    command
        .arg(CHECK_FLAG)
        .arg(path)
        // An inherited stdout would hold Claude Code until the check finished
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // Its own process group, so Claude Code cancelling this render doesn't kill the check
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    let _ = command.spawn();
}

/// Look up the latest release tag and cache it at `path`.
pub fn check(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    // Follow the /releases/latest redirect, as install.sh does: the API
    // rate-limits unauthenticated callers to 60 requests an hour.
    let latest = format!("{}/releases/latest", env!("CARGO_PKG_REPOSITORY"));
    let output = Command::new("curl")
        .args(["-fsSLI", "--max-time", "30", "-o", "/dev/null"])
        .args(["-w", "%{url_effective}", &latest])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()?;
    if !output.status.success() {
        return Err(format!("curl exited with {}", output.status).into());
    }
    let url = String::from_utf8(output.stdout)?;
    let tag = url
        .rsplit_once("/tag/")
        .map(|(_, tag)| tag.trim())
        .filter(|tag| parse_version(tag).is_some())
        .ok_or_else(|| format!("no release tag in {url:?}"))?;

    // Write aside, then rename, so a render never reads a half-written tag
    let partial = path.with_extension("update.partial");
    fs::write(&partial, format!("{tag}\n"))?;
    fs::rename(&partial, path)?;
    Ok(())
}

/// `v1.2.3` or `1.2.3`. Anything else, pre-releases included, is `None`.
pub fn parse_version(text: &str) -> Option<(u64, u64, u64)> {
    let mut parts = text
        .strip_prefix('v')
        .unwrap_or(text)
        .split('.')
        .map(|part| {
            // `parse` alone would also take a leading `+`
            if part.bytes().all(|b| b.is_ascii_digit()) {
                part.parse::<u64>().ok()
            } else {
                None
            }
        });
    let version = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(version)
}

/// `↑ v1.2.3`, linked to the README's update steps, when `latest` is newer than `current`.
pub fn notice(latest: &str, current: &str) -> Option<Section> {
    if parse_version(latest)? <= parse_version(current)? {
        return None;
    }
    let url = format!("{}#install-or-update", env!("CARGO_PKG_REPOSITORY"));
    Some(Section::new(format!("\u{2191} {latest}"), ORANGE, WHITE).link(Some(url)))
}
