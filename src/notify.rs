//! Desktop notification shortly before the prompt cache expires.
//!
//! Off unless `CLAUDE_STATUSLINE_CACHE_NOTIFY` sets a lead time in minutes. The first render
//! inside that window claims a marker named after the expiry and the session, so the renders
//! that follow on `refreshInterval` stay quiet. The next request moves the expiry, which
//! re-arms it. The notifier runs detached, like the update check.

use crate::update::spawn_detached;
use std::fs;
use std::path::Path;
use std::process::Command;

/// In Claude Code's config directory, next to the update check's cache.
pub const MARKER_DIR: &str = "claude_statusline.cache-notify";

/// Lead time in seconds, from whole minutes. Off when unset, zero or not a number.
pub fn lead_secs(var: impl Fn(&str) -> Option<String>) -> Option<u64> {
    let minutes = var("CLAUDE_STATUSLINE_CACHE_NOTIFY")?
        .trim()
        .parse::<u64>()
        .ok()?;
    minutes.checked_mul(60).filter(|&secs| secs > 0)
}

/// True for the first caller per expiry and session, which then prunes markers of
/// caches that have expired since.
pub fn claim(dir: &Path, expires_at: u64, session_id: &str, now: u64) -> bool {
    // Session ids are UUIDs; keep whatever else arrives out of the path
    let session: String = session_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    let claimed = fs::create_dir_all(dir)
        .and_then(|()| fs::File::create_new(dir.join(format!("{expires_at}-{session}"))))
        .is_ok();
    if claimed {
        prune(dir, now);
    }
    claimed
}

fn prune(dir: &Path, now: u64) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let expired = name
            .to_str()
            .and_then(|name| name.split_once('-'))
            .and_then(|(expires_at, _)| expires_at.parse::<u64>().ok())
            .is_some_and(|expires_at| expires_at <= now);
        if expired {
            let _ = fs::remove_file(entry.path());
        }
    }
}

/// Show a notification without waiting for it: `osascript` on macOS, `notify-send` elsewhere.
pub fn send(title: &str, body: &str) {
    let mut command;
    if cfg!(target_os = "macos") {
        command = Command::new("osascript");
        // Passed as arguments, so quotes in the text need no escaping
        command.args([
            "-e",
            "on run argv",
            "-e",
            "display notification (item 2 of argv) with title (item 1 of argv)",
            "-e",
            "end run",
        ]);
    } else {
        command = Command::new("notify-send");
        command.arg("--app-name=Claude Code");
    }
    command.args([title, body]);
    spawn_detached(command);
}
