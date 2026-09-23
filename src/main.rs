mod lenient;
mod render;

use render::{
    BLUE, CYAN, GRAY, GREEN, INDIGO, OLIVE, ORANGE, PURPLE, RED, SLATE, Section, TEAL, WHITE,
    YELLOW, fmt_duration_ms, fmt_duration_secs, fmt_tokens, format_row, unix_now, usage_colors,
};
use serde::Deserialize;
use std::fmt::Write as _;
use std::io::Read;
use std::path::Path;
use std::process::Command;

#[derive(Deserialize)]
struct Input {
    #[serde(default, deserialize_with = "lenient::option")]
    session_name: Option<String>,
    workspace: Workspace,
    model: Model,
    #[serde(default, deserialize_with = "lenient::option")]
    cost: Option<Cost>,
    #[serde(default, deserialize_with = "lenient::option")]
    context_window: Option<ContextWindow>,
    #[serde(default, deserialize_with = "lenient::option")]
    exceeds_200k_tokens: Option<bool>,
    #[serde(default, deserialize_with = "lenient::option")]
    fast_mode: Option<bool>,
    #[serde(default, deserialize_with = "lenient::option")]
    effort: Option<Effort>,
    #[serde(default, deserialize_with = "lenient::option")]
    thinking: Option<Thinking>,
    #[serde(default, deserialize_with = "lenient::option")]
    rate_limits: Option<RateLimits>,
    #[serde(default, deserialize_with = "lenient::option")]
    prompt_cache: Option<PromptCache>,
    #[serde(default, deserialize_with = "lenient::option")]
    agent: Option<Agent>,
    #[serde(default, deserialize_with = "lenient::option")]
    pr: Option<Pr>,
    #[serde(default, deserialize_with = "lenient::option")]
    worktree: Option<Worktree>,
}

#[derive(Deserialize)]
struct Workspace {
    current_dir: String,
    /// Present when the current directory sits inside a linked git worktree.
    #[serde(default, deserialize_with = "lenient::option")]
    git_worktree: Option<String>,
    /// Present inside a git repository with an `origin` remote.
    #[serde(default, deserialize_with = "lenient::option")]
    repo: Option<Repo>,
}

#[derive(Deserialize)]
struct Repo {
    #[serde(default, deserialize_with = "lenient::option")]
    host: Option<String>,
    #[serde(default, deserialize_with = "lenient::option")]
    owner: Option<String>,
    #[serde(default, deserialize_with = "lenient::option")]
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
    #[serde(default, deserialize_with = "lenient::option")]
    total_cost_usd: Option<f64>,
    #[serde(default, deserialize_with = "lenient::option")]
    total_duration_ms: Option<u64>,
    #[serde(default, deserialize_with = "lenient::option")]
    total_lines_added: Option<u64>,
    #[serde(default, deserialize_with = "lenient::option")]
    total_lines_removed: Option<u64>,
}

#[allow(clippy::struct_field_names)]
#[derive(Deserialize)]
struct ContextWindow {
    #[serde(default, deserialize_with = "lenient::option")]
    total_input_tokens: Option<u64>,
    #[serde(default, deserialize_with = "lenient::option")]
    context_window_size: Option<u64>,
    #[serde(default, deserialize_with = "lenient::option")]
    used_percentage: Option<f64>,
    /// Null before the first API call, and again after `/compact`.
    #[serde(default, deserialize_with = "lenient::option")]
    current_usage: Option<CurrentUsage>,
}

#[derive(Deserialize)]
struct CurrentUsage {
    #[serde(default, deserialize_with = "lenient::option")]
    cache_read_input_tokens: Option<u64>,
}

#[derive(Deserialize)]
struct Effort {
    #[serde(default, deserialize_with = "lenient::option")]
    level: Option<String>,
}

#[derive(Deserialize)]
struct Thinking {
    #[serde(default, deserialize_with = "lenient::option")]
    enabled: Option<bool>,
}

/// Claude.ai subscribers only, after the first API response.
#[derive(Deserialize)]
struct RateLimits {
    #[serde(default, deserialize_with = "lenient::option")]
    five_hour: Option<RateWindow>,
    #[serde(default, deserialize_with = "lenient::option")]
    seven_day: Option<RateWindow>,
    /// Behind a Claude apps gateway only; may exceed 100%.
    #[serde(default, deserialize_with = "lenient::option")]
    spend_limit: Option<RateWindow>,
}

/// Main-conversation prompt cache statistics, after the first API response.
#[derive(Deserialize)]
struct PromptCache {
    #[serde(default, deserialize_with = "lenient::option")]
    warm: Option<bool>,
    /// False when caching is off or the provider doesn't report it.
    #[serde(default, deserialize_with = "lenient::option")]
    caching_observed: Option<bool>,
    #[serde(default, deserialize_with = "lenient::option")]
    ttl: Option<String>,
    /// Null while cold.
    #[serde(default, deserialize_with = "lenient::option")]
    expires_at: Option<u64>,
    /// Cache reads as a fraction of all input, 0 to 1.
    #[serde(default, deserialize_with = "lenient::option")]
    hit_ratio: Option<f64>,
    /// Null right after a compaction until the next request.
    #[serde(default, deserialize_with = "lenient::option")]
    recache_tokens_if_cold: Option<u64>,
}

#[derive(Deserialize)]
struct RateWindow {
    #[serde(default, deserialize_with = "lenient::option")]
    used_percentage: Option<f64>,
    #[serde(default, deserialize_with = "lenient::option")]
    resets_at: Option<u64>,
}

#[derive(Deserialize)]
struct Agent {
    #[serde(default, deserialize_with = "lenient::option")]
    name: Option<String>,
}

#[derive(Deserialize)]
struct Pr {
    #[serde(default, deserialize_with = "lenient::option")]
    number: Option<u64>,
    #[serde(default, deserialize_with = "lenient::option")]
    url: Option<String>,
    #[serde(default, deserialize_with = "lenient::option")]
    review_state: Option<String>,
    /// `mr` for a GitLab merge request, absent for GitHub.
    #[serde(default, deserialize_with = "lenient::option")]
    kind: Option<String>,
}

#[derive(Deserialize)]
struct Worktree {
    #[serde(default, deserialize_with = "lenient::option")]
    name: Option<String>,
    #[serde(default, deserialize_with = "lenient::option")]
    branch: Option<String>,
    #[serde(default, deserialize_with = "lenient::option")]
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
        sections.push(Section::new(format!("\u{e0a0} {branch}"), GREEN, WHITE));
    }

    // Worktree: `--worktree` session, or a plain linked git worktree
    let worktree_name = input
        .worktree
        .as_ref()
        .and_then(|w| w.name.clone())
        .or_else(|| input.workspace.git_worktree.clone());
    if let Some(name) = worktree_name {
        let mut text = format!("\u{29c9} {name}");
        if let Some(original) = input
            .worktree
            .as_ref()
            .and_then(|w| w.original_branch.as_ref())
        {
            let _ = write!(text, " \u{2190} {original}");
        }
        sections.push(Section::new(text, TEAL, WHITE));
    }

    // Open pull request for the current branch
    if let Some(pr) = &input.pr {
        let is_mr = pr.kind.as_deref() == Some("mr");
        let mut text = match (pr.number, is_mr) {
            (Some(number), true) => format!("MR !{number}"),
            (Some(number), false) => format!("PR #{number}"),
            (None, true) => "MR".to_string(),
            (None, false) => "PR".to_string(),
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
///
/// `now` is Unix epoch seconds, passed in so the countdowns are deterministic under test.
fn session_sections(input: &Input, now: u64) -> (Vec<Section>, Vec<Section>) {
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

    if let Some(cache) = &input.prompt_cache {
        sections.extend(prompt_cache_section(cache, now));
    }

    // Subscription rate limits
    if let Some(limits) = &input.rate_limits {
        if let Some(window) = &limits.five_hour {
            right.extend(rate_limit_section("5h", window, now));
        }
        if let Some(window) = &limits.seven_day {
            right.extend(rate_limit_section("7d", window, now));
        }
        if let Some(window) = &limits.spend_limit {
            right.extend(rate_limit_section("spend", window, now));
        }
    }

    (sections, right)
}

fn rate_limit_section(label: &str, window: &RateWindow, now: u64) -> Option<Section> {
    let pct = window.used_percentage?;
    let mut text = format!("{label} {}%", pct.round() as u64);

    if let Some(resets_at) = window.resets_at
        && resets_at > now
    {
        let _ = write!(text, " ({})", fmt_duration_secs(resets_at - now));
    }

    let (bg, fg) = usage_colors(pct);
    Some(Section::new(text, bg, fg))
}

/// Cache countdown: seconds in the last minute, whole minutes before that.
///
/// Between events the line only redraws on `refreshInterval`, so seconds would be false precision.
fn fmt_cache_countdown(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else {
        format!("{}m", secs / 60)
    }
}

/// Warm: hit ratio and time until the cached prefix expires. Cold: what the next request re-caches.
fn prompt_cache_section(cache: &PromptCache, now: u64) -> Option<Section> {
    if cache.caching_observed != Some(true) {
        return None;
    }

    // The payload can be stale by the time it renders, so check expiry locally too
    let remaining = cache
        .expires_at
        .filter(|&expires_at| cache.warm == Some(true) && expires_at > now)
        .map(|expires_at| expires_at - now);

    let Some(remaining) = remaining else {
        let mut text = "cache cold".to_string();
        if let Some(tokens) = cache.recache_tokens_if_cold
            && tokens > 0
        {
            let _ = write!(text, " \u{21bb}{}", fmt_tokens(tokens));
        }
        return Some(Section::new(text, SLATE, WHITE));
    };

    let mut text = "cache".to_string();
    if let Some(ratio) = cache.hit_ratio {
        let _ = write!(text, " {}%", (ratio * 100.0).round() as u64);
    }
    let _ = write!(text, " {}", fmt_cache_countdown(remaining));

    // Yellow in the last fifth of the TTL: send soon or pay to rebuild
    let ttl_secs = match cache.ttl.as_deref() {
        Some("1h") => 3_600,
        _ => 300,
    };
    let (bg, fg) = if remaining * 5 <= ttl_secs {
        (YELLOW, render::BLACK)
    } else {
        (CYAN, WHITE)
    };
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

    let (left, right) = session_sections(&input, unix_now());
    if !left.is_empty() || !right.is_empty() {
        println!("{}", format_row(left, right));
    }

    Ok(())
}

fn main() {
    // Claude Code passes no arguments, so this never shadows a real render
    if std::env::args()
        .nth(1)
        .is_some_and(|arg| arg == "--version" || arg == "-V")
    {
        println!("{} {}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION"));
        return;
    }

    // Keep a visible placeholder on screen; the reason goes to stderr, which `claude --debug` logs.
    if let Err(error) = run() {
        eprintln!("claude_statusline: {error}");
        println!("[statusline]");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use render::{BLACK, format_row_within, group_width, strip_ansi, truncate};
    use serde_json::{Value, json};
    use unicode_width::UnicodeWidthStr as _;

    const NOW: u64 = 1_750_000_000;

    /// The two required fields, plus `extra` merged in at the top level.
    fn input(extra: Value) -> Input {
        let mut payload = json!({
            // Nonexistent, so the git fallback finds nothing
            "workspace": { "current_dir": "/nonexistent/demo" },
            "model": { "display_name": "Opus" },
        });
        if let (Value::Object(base), Value::Object(extra)) = (&mut payload, extra) {
            base.extend(extra);
        }
        serde_json::from_value(payload).expect("payload parses")
    }

    fn texts(sections: &[Section]) -> Vec<&str> {
        sections.iter().map(|s| s.text.as_str()).collect()
    }

    fn cache(fields: Value) -> Section {
        let mut extra = json!({});
        extra["prompt_cache"] = fields;
        let input = input(extra);
        prompt_cache_section(input.prompt_cache.as_ref().expect("prompt_cache"), NOW)
            .expect("cache section")
    }

    #[test]
    fn warm_cache_shows_hit_ratio_and_whole_minutes() {
        let section = cache(json!({
            "warm": true, "caching_observed": true, "ttl": "1h",
            "expires_at": NOW + 2_520, "hit_ratio": 0.91,
        }));
        assert_eq!(section.text, "cache 91% 42m");
        assert_eq!(section.bg, CYAN);
    }

    #[test]
    fn cache_turns_yellow_in_the_last_fifth_of_its_ttl() {
        let at = |remaining: u64| {
            cache(json!({
                "warm": true, "caching_observed": true, "ttl": "5m",
                "expires_at": NOW + remaining,
            }))
        };
        assert_eq!(at(61).bg, CYAN);
        assert_eq!(at(60).bg, YELLOW);
        assert_eq!(at(60).text, "cache 1m");
        assert_eq!(at(40).text, "cache 40s");
        assert_eq!(at(40).fg, BLACK);
    }

    #[test]
    fn expired_cache_is_cold_even_when_the_payload_says_warm() {
        let section = cache(json!({
            "warm": true, "caching_observed": true, "ttl": "5m",
            "expires_at": NOW - 10, "hit_ratio": 0.8, "recache_tokens_if_cold": 61_000,
        }));
        assert_eq!(section.text, "cache cold \u{21bb}61k");
        assert_eq!(section.bg, SLATE);

        let unknown_size =
            cache(json!({ "warm": false, "caching_observed": true, "expires_at": null }));
        assert_eq!(unknown_size.text, "cache cold");
    }

    #[test]
    fn cache_is_hidden_when_caching_is_not_observed() {
        let input = input(json!({ "prompt_cache": { "warm": false, "caching_observed": false } }));
        assert!(prompt_cache_section(input.prompt_cache.as_ref().unwrap(), NOW).is_none());
    }

    #[test]
    fn rate_limits_show_time_until_reset_and_spend_can_pass_100() {
        let input = input(json!({ "rate_limits": {
            "five_hour": { "used_percentage": 23.5, "resets_at": NOW + 7_980 },
            "seven_day": { "used_percentage": 41.2, "resets_at": NOW - 1 },
            "spend_limit": { "used_percentage": 104.2, "resets_at": NOW + 12 * 86_400 },
        }}));
        let (_, right) = session_sections(&input, NOW);
        assert_eq!(
            texts(&right),
            ["5h 24% (2h13m)", "7d 41%", "spend 104% (12d0h)"]
        );
        assert_eq!(right[2].bg, RED);
    }

    #[test]
    fn model_section_carries_fast_thinking_and_effort_markers() {
        let input = input(json!({
            "fast_mode": true, "thinking": { "enabled": true }, "effort": { "level": "high" },
            "cost": { "total_cost_usd": 0.004, "total_duration_ms": 999 },
        }));
        let (left, right) = session_sections(&input, NOW);
        assert_eq!(texts(&left), ["Opus \u{26a1} \u{273b} high"]);
        // Sub-cent cost and sub-second duration are noise
        assert!(right.is_empty());
    }

    #[test]
    fn gitlab_merge_requests_use_the_mr_label() {
        let mr = input(json!({ "pr": { "number": 42, "kind": "mr", "review_state": "draft" } }));
        let (left, _) = location_sections(&mr);
        assert_eq!(texts(&left), ["demo", "MR !42 draft"]);
        assert_eq!(left[1].bg, SLATE);

        let pr = input(json!({ "pr": { "number": 7, "review_state": "approved" } }));
        let (left, _) = location_sections(&pr);
        assert_eq!(left[1].text, "PR #7 approved");
        assert_eq!(left[1].bg, GREEN);
    }

    #[test]
    fn fields_with_unexpected_types_are_dropped_not_fatal() {
        let input = input(json!({
            "cost": "oops",
            "fast_mode": "yes",
            "effort": { "level": 3 },
            "prompt_cache": [1, 2],
            "context_window": {
                "used_percentage": 37, "total_input_tokens": "many", "context_window_size": 200_000,
            },
            "rate_limits": {
                "five_hour": { "used_percentage": "high", "resets_at": NOW + 60 },
                "seven_day": { "used_percentage": 41.2, "resets_at": 1.5e9 },
            },
            "pr": { "number": "42", "url": "https://example.com/pr/42" },
        }));
        let (left, right) = session_sections(&input, NOW);
        assert_eq!(texts(&left), ["Opus", "37%"]);
        assert_eq!(texts(&right), ["7d 41%"]);

        let (left, _) = location_sections(&input);
        assert_eq!(texts(&left), ["demo", "PR"]);
    }

    #[test]
    fn required_fields_stay_strict() {
        let missing_model = json!({ "workspace": { "current_dir": "/tmp" } });
        assert!(serde_json::from_value::<Input>(missing_model).is_err());
    }

    #[test]
    fn right_group_is_pushed_against_the_usable_width() {
        let row = |usable| {
            let left = vec![Section::new("Opus", GRAY, WHITE)];
            let right = vec![Section::new("$0.42", PURPLE, WHITE)];
            strip_ansi(&format_row_within(left, right, usable))
        };
        assert_eq!(row(Some(40)).width(), 40);
        assert!(row(Some(40)).contains("   "));

        // Too narrow, or width unknown: one continuous powerline, no padding
        let continuous = group_width(&[
            Section::new("Opus", GRAY, WHITE),
            Section::new("$0.42", PURPLE, WHITE),
        ]);
        assert_eq!(row(Some(10)).width(), continuous);
        assert_eq!(row(None).width(), continuous);
    }

    #[test]
    fn compact_formatters() {
        assert_eq!(fmt_tokens(950), "950");
        assert_eq!(fmt_tokens(74_500), "74k");
        assert_eq!(fmt_tokens(1_250_000), "1.2M");

        assert_eq!(fmt_duration_secs(45), "45s");
        assert_eq!(fmt_duration_secs(754), "12m34s");
        assert_eq!(fmt_duration_secs(7_980), "2h13m");
        assert_eq!(fmt_duration_secs(3 * 86_400 + 5 * 3_600), "3d5h");

        assert_eq!(fmt_cache_countdown(59), "59s");
        assert_eq!(fmt_cache_countdown(3_600), "60m");

        assert_eq!(truncate("hello world", 5), "hell\u{2026}");
        assert_eq!(truncate("hi", 5), "hi");
        assert_eq!(truncate("hello", 1), "\u{2026}");
        // Wide characters take two cells each
        assert_eq!(truncate("日本語テキスト", 5), "日本\u{2026}");
        assert_eq!(
            truncate("\u{1f680}\u{1f680}\u{1f680}", 4),
            "\u{1f680}\u{2026}"
        );
        assert_eq!(truncate("日本", 4), "日本");
    }

    #[test]
    fn usage_color_thresholds() {
        assert_eq!(usage_colors(59.0).0, GREEN);
        assert_eq!(usage_colors(59.5).0, YELLOW);
        assert_eq!(usage_colors(80.0).0, YELLOW);
        assert_eq!(usage_colors(80.5).0, RED);
    }
}
