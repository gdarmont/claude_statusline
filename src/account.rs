//! The Claude account Claude Code is logged in to, for telling several accounts apart.
//!
//! Off unless `CLAUDE_STATUSLINE_ACCOUNT` asks for it. The payload doesn't carry the account, so
//! it comes from the `oauthAccount` that Claude Code keeps in its global config file. That file is
//! read on every render, so a `/login` to another account shows on the next one.

use crate::lenient;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Show {
    Email,
    Org,
}

/// `org` for the organization, any other value for the email. Off when unset, empty or off.
pub fn show(var: impl Fn(&str) -> Option<String>) -> Option<Show> {
    match var("CLAUDE_STATUSLINE_ACCOUNT")?
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "" | "0" | "false" | "no" | "off" => None,
        "org" => Some(Show::Org),
        _ => Some(Show::Email),
    }
}

/// Where Claude Code may keep its global config, in the order it looks: a legacy `.config.json`
/// in its config directory, then `.claude.json` in `$CLAUDE_CONFIG_DIR` or the home directory.
pub fn config_files(var: impl Fn(&str) -> Option<String>) -> Vec<PathBuf> {
    let non_empty = |name: &str| var(name).filter(|value| !value.is_empty());
    let config_dir = non_empty("CLAUDE_CONFIG_DIR");
    let home = non_empty("HOME");

    let mut files = Vec::new();
    if let Some(dir) = config_dir
        .clone()
        .map(PathBuf::from)
        .or_else(|| home.as_ref().map(|home| Path::new(home).join(".claude")))
    {
        files.push(dir.join(".config.json"));
    }
    // Claude Code keeps a custom OAuth server's login apart from the default one
    let suffix = if non_empty("CLAUDE_CODE_CUSTOM_OAUTH_URL").is_some() {
        "-custom-oauth"
    } else {
        ""
    };
    if let Some(dir) = config_dir.or(home) {
        files.push(Path::new(&dir).join(format!(".claude{suffix}.json")));
    }
    files
}

/// Only the login: serde skips the rest of the file without building it.
#[derive(Deserialize)]
struct GlobalConfig {
    #[serde(rename = "oauthAccount", default, deserialize_with = "lenient::option")]
    oauth_account: Option<OauthAccount>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OauthAccount {
    #[serde(default, deserialize_with = "lenient::option")]
    email_address: Option<String>,
    #[serde(default, deserialize_with = "lenient::option")]
    organization_name: Option<String>,
}

/// The account in the first of `files` that has any content. `None` when there is no login,
/// as with an API key or a cloud provider, or when the file can't be read or parsed.
pub fn label(files: &[PathBuf], show: Show) -> Option<String> {
    let text = files.iter().find_map(|file| {
        fs::read_to_string(file)
            .ok()
            .filter(|text| !text.trim().is_empty())
    })?;
    let config: GlobalConfig = serde_json::from_str(&text).ok()?;
    pick(config.oauth_account?, show)
}

fn pick(account: OauthAccount, show: Show) -> Option<String> {
    let clean = |text: Option<String>| {
        text.map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty())
    };
    let email = clean(account.email_address);
    let org = clean(account.organization_name);
    match show {
        Show::Email => email.or(org),
        Show::Org => match (org, email) {
            // A personal plan's organization is named after its email, so the email says it all
            (Some(org), Some(email)) if org == format!("{email}'s Organization") => Some(email),
            (org, email) => org.or(email),
        },
    }
}
