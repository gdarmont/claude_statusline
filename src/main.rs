mod render;

use render::{
    BLUE, GRAY, GREEN, INDIGO, OLIVE, ORANGE, PURPLE, RED, SLATE, Section, TEAL, WHITE, YELLOW,
    fmt_duration_ms, fmt_duration_secs, fmt_tokens, format_row, unix_now, usage_colors,
};
use serde::Deserialize;
use std::fmt::Write as _;
use std::io::Read;
use std::path::Path;
use std::process::Command;

#[derive(Deserialize)]
struct Input {
    session_name: Option<String>,
    workspace: Workspace,
    model: Model,
    cost: Option<Cost>,
    context_window: Option<ContextWindow>,
    exceeds_200k_tokens: Option<bool>,
    fast_mode: Option<bool>,
    effort: Option<Effort>,
    thinking: Option<Thinking>,
    rate_limits: Option<RateLimits>,
    agent: Option<Agent>,
    pr: Option<Pr>,
    worktree: Option<Worktree>,
}

#[derive(Deserialize)]
struct Workspace {
    current_dir: String,
    /// Present when the current directory sits inside a linked git worktree.
    git_worktree: Option<String>,
    /// Present inside a git repository with an `origin` remote.
    repo: Option<Repo>,
}

#[derive(Deserialize)]
struct Repo {
    host: Option<String>,
    owner: Option<String>,
    name: Option<String>,
}

#[derive(Deserialize)]
struct Model {
    display_name: String,
}

// Field names mirror the wire format, prefixes included.
#[allow(clippy::struct_field_names)]
#[derive(Deserialize)]
struct Cost {
    total_cost_usd: Option<f64>,
    total_duration_ms: Option<u64>,
    total_lines_added: Option<u64>,
    total_lines_removed: Option<u64>,
}

#[allow(clippy::struct_field_names)]
#[derive(Deserialize)]
struct ContextWindow {
    total_input_tokens: Option<u64>,
    context_window_size: Option<u64>,
    used_percentage: Option<f64>,
    /// Null before the first API call, and again after `/compact`.
    current_usage: Option<CurrentUsage>,
}

#[derive(Deserialize)]
struct CurrentUsage {
    cache_read_input_tokens: Option<u64>,
}

#[derive(Deserialize)]
struct Effort {
    level: Option<String>,
}

#[derive(Deserialize)]
struct Thinking {
    enabled: Option<bool>,
}

/// Claude.ai subscribers only, after the first API response.
#[derive(Deserialize)]
struct RateLimits {
    five_hour: Option<RateWindow>,
    seven_day: Option<RateWindow>,
}

#[derive(Deserialize)]
struct RateWindow {
    used_percentage: Option<f64>,
    resets_at: Option<u64>,
}

#[derive(Deserialize)]
struct Agent {
    name: Option<String>,
}

#[derive(Deserialize)]
struct Pr {
    number: Option<u64>,
    url: Option<String>,
    review_state: Option<String>,
}

#[derive(Deserialize)]
struct Worktree {
    name: Option<String>,
    branch: Option<String>,
    original_branch: Option<String>,
}

fn get_git_branch(dir: &str) -> Option<String> {
    let git = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .current_dir(dir)
            .stderr(std::process::Stdio::null())
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
    };

    git(&["branch", "--show-current"]).or_else(|| {
        // Detached HEAD: fall back to the short commit hash
        git(&["rev-parse", "--short", "HEAD"]).map(|hash| format!("HEAD@{hash}"))
    })
}

/// First row: where the work is happening. Session name sits on the right.
fn location_sections(input: &Input) -> (Vec<Section>, Vec<Section>) {
    let mut sections = Vec::new();

    // Directory name, linked to the remote repository when one is known
    let dir_name = Path::new(&input.workspace.current_dir)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&input.workspace.current_dir);
    let repo_url = input.workspace.repo.as_ref().and_then(|repo| {
        match (&repo.host, &repo.owner, &repo.name) {
            (Some(host), Some(owner), Some(name)) => Some(format!("https://{host}/{owner}/{name}")),
            _ => None,
        }
    });
    sections.push(Section::new(dir_name, BLUE, WHITE).link(repo_url));

    // Git branch: from the worktree payload when available, else ask git
    let branch = input
        .worktree
        .as_ref()
        .and_then(|w| w.branch.clone())
        .or_else(|| get_git_branch(&input.workspace.current_dir));
    if let Some(branch) = branch {
        sections.push(Section::new(
            format!("\u{e0a0} {branch}"),
            GREEN,
            WHITE,
        ));
    }

    // Worktree: `--worktree` session, or a plain linked git worktree
    let worktree_name = input
        .worktree
        .as_ref()
        .and_then(|w| w.name.clone())
        .or_else(|| input.workspace.git_worktree.clone());
    if let Some(name) = worktree_name {
        let mut text = format!("\u{29c9} {name}");
        if let Some(original) = input.worktree.as_ref().and_then(|w| w.original_branch.as_ref()) {
            let _ = write!(text, " \u{2190} {original}");
        }
        sections.push(Section::new(text, TEAL, WHITE));
    }

    // Open pull request for the current branch
    if let Some(pr) = &input.pr {
        let mut text = match pr.number {
            Some(number) => format!("PR #{number}"),
            None => "PR".to_string(),
        };
        let (bg, fg) = match pr.review_state.as_deref() {
            Some("approved") => (GREEN, WHITE),
            Some("changes_requested") => (RED, WHITE),
            Some("pending") => (YELLOW, render::BLACK),
            _ => (SLATE, WHITE),
        };
        if let Some(state) = &pr.review_state {
            let _ = write!(text, " {}", state.replace('_', " "));
        }
        sections.push(Section::new(text, bg, fg).link(pr.url.clone()));
    }

    // Session name, when one was set explicitly or generated
    let right = input
        .session_name
        .as_ref()
        .map(|name| Section::new(name.clone(), SLATE, WHITE))
        .into_iter()
        .collect();

    (sections, right)
}

/// Second row: model state and context on the left, spend and rate limits on the right.
fn session_sections(input: &Input) -> (Vec<Section>, Vec<Section>) {
    let mut sections = Vec::new();
    // Pinned to the right edge, so they hold a fixed position as the left side grows
    let mut right = Vec::new();

    // Model, with fast mode / thinking / effort markers
    let mut model_text = input.model.display_name.clone();
    if input.fast_mode == Some(true) {
        model_text.push_str(" \u{26a1}");
    }
    if input.thinking.as_ref().and_then(|t| t.enabled) == Some(true) {
        model_text.push_str(" \u{273b}");
    }
    if let Some(level) = input.effort.as_ref().and_then(|e| e.level.as_ref()) {
        let _ = write!(model_text, " {level}");
    }
    sections.push(Section::new(model_text, GRAY, WHITE));

    // Named agent (`--agent` or agent settings)
    if let Some(name) = input.agent.as_ref().and_then(|a| a.name.as_ref()) {
        sections.push(Section::new(name.clone(), ORANGE, WHITE));
    }

    if let Some(cost) = &input.cost {
        // Session cost
        if let Some(usd) = cost.total_cost_usd
            && usd >= 0.01
        {
            right.push(Section::new(format!("${usd:.2}"), PURPLE, WHITE));
        }

        // Lines changed this session
        let added = cost.total_lines_added.unwrap_or(0);
        let removed = cost.total_lines_removed.unwrap_or(0);
        if added > 0 || removed > 0 {
            sections.push(Section::new(format!("+{added}/-{removed}"), OLIVE, WHITE));
        }

        // Wall-clock session duration
        if let Some(ms) = cost.total_duration_ms
            && ms >= 1_000
        {
            right.push(Section::new(fmt_duration_ms(ms), INDIGO, WHITE));
        }
    }

    // Context window usage
    if let Some(cw) = &input.context_window
        && let Some(pct) = cw.used_percentage
    {
        let (bg, fg) = usage_colors(pct);
        let mut text = format!("{}%", pct.round() as u64);
        if let (Some(used), Some(size)) = (cw.total_input_tokens, cw.context_window_size)
            && size > 0
        {
            let _ = write!(text, " {}/{}", fmt_tokens(used), fmt_tokens(size));
        }
        if let Some(cached) = cw
            .current_usage
            .as_ref()
            .and_then(|u| u.cache_read_input_tokens)
            && cached > 0
        {
            let _ = write!(text, " \u{21ba}{}", fmt_tokens(cached));
        }
        if input.exceeds_200k_tokens == Some(true) {
            text.push_str(" 200k+");
        }
        sections.push(Section::new(text, bg, fg));
    }

    // Subscription rate limits
    if let Some(limits) = &input.rate_limits {
        if let Some(window) = &limits.five_hour {
            right.extend(rate_limit_section("5h", window));
        }
        if let Some(window) = &limits.seven_day {
            right.extend(rate_limit_section("7d", window));
        }
    }

    (sections, right)
}

fn rate_limit_section(label: &str, window: &RateWindow) -> Option<Section> {
    let pct = window.used_percentage?;
    let mut text = format!("{label} {}%", pct.round() as u64);

    if let Some(resets_at) = window.resets_at {
        let now = unix_now();
        if resets_at > now {
            let _ = write!(text, " ({})", fmt_duration_secs(resets_at - now));
        }
    }

    let (bg, fg) = usage_colors(pct);
    Some(Section::new(text, bg, fg))
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut input_str = String::new();
    std::io::stdin().read_to_string(&mut input_str)?;
    let input: Input = serde_json::from_str(&input_str)?;

    let (left, right) = location_sections(&input);
    if !left.is_empty() || !right.is_empty() {
        println!("{}", format_row(left, right));
    }

    let (left, right) = session_sections(&input);
    if !left.is_empty() || !right.is_empty() {
        println!("{}", format_row(left, right));
    }

    Ok(())
}

fn main() {
    if run().is_err() {
        println!("[statusline]");
    }
}
