//! Renders one row per visible subagent for the `subagentStatusLine` setting.
//!
//! Reads every visible task as a single JSON object on stdin and writes one
//! `{"id": ..., "content": ...}` line per row it wants to override.

#[path = "../lenient.rs"]
mod lenient;
#[path = "../render.rs"]
mod render;

use render::{
    BLUE, GREEN, RED, Rgb, SLATE, WHITE, YELLOW, fg_styled, fmt_duration_secs, fmt_tokens,
    truncate, unix_now, usage_colors,
};
use serde::Deserialize;
use std::io::Read;
use unicode_width::UnicodeWidthStr as _;

const DIM: Rgb = Rgb(120, 124, 138);
const SEPARATOR: &str = " \u{b7} ";
/// Below this the detail column is dropped rather than truncated to noise.
const MIN_DETAIL_WIDTH: usize = 8;
const SPARK_BARS: [char; 8] = [
    '\u{2581}', '\u{2582}', '\u{2583}', '\u{2584}', '\u{2585}', '\u{2586}', '\u{2587}', '\u{2588}',
];
/// Intervals the sparkline covers, about five seconds each.
const SPARK_WIDTH: usize = 8;

#[derive(Deserialize)]
struct Input {
    /// Usable row width, as reported by Claude Code.
    #[serde(default, deserialize_with = "lenient::option")]
    columns: Option<usize>,
    /// A task that fails to parse keeps its default rendering; the others still render.
    #[serde(default, deserialize_with = "lenient::vec")]
    tasks: Vec<Task>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Task {
    id: String,
    #[serde(default, deserialize_with = "lenient::option")]
    name: Option<String>,
    #[serde(default, deserialize_with = "lenient::option")]
    status: Option<String>,
    #[serde(default, deserialize_with = "lenient::option")]
    description: Option<String>,
    #[serde(default, deserialize_with = "lenient::option")]
    label: Option<String>,
    /// Resolved model ID (Claude Code v2.1.205+).
    #[serde(default, deserialize_with = "lenient::option")]
    model: Option<String>,
    /// Effort level string, or a numeric token budget (v2.1.214+).
    #[serde(default, deserialize_with = "lenient::option")]
    effort: Option<Effort>,
    #[serde(default, deserialize_with = "lenient::option")]
    context_window_size: Option<u64>,
    #[serde(default, deserialize_with = "lenient::option")]
    token_count: Option<u64>,
    /// Unix milliseconds.
    #[serde(default, deserialize_with = "lenient::option")]
    start_time: Option<u64>,
    /// `tokenCount` on recent refresh ticks, oldest first: Claude Code keeps the last 16.
    #[serde(default, deserialize_with = "lenient::vec")]
    token_samples: Vec<u64>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Effort {
    Level(String),
    Budget(u64),
}

impl Effort {
    fn label(&self) -> String {
        match self {
            Self::Level(level) => level.clone(),
            Self::Budget(tokens) => fmt_tokens(*tokens),
        }
    }
}

fn is_running(status: Option<&str>) -> bool {
    matches!(status, Some("running" | "in_progress" | "active"))
}

/// Marker glyph and color for a task status.
fn status_marker(status: Option<&str>) -> (&'static str, Rgb) {
    match status {
        _ if is_running(status) => ("\u{25cf}", GREEN),
        Some("completed" | "done" | "success") => ("\u{2713}", BLUE),
        Some("failed" | "error" | "cancelled" | "killed") => ("\u{2717}", RED),
        Some("pending" | "queued" | "waiting") => ("\u{25cb}", YELLOW),
        _ => ("\u{25cf}", SLATE),
    }
}

/// `claude-opus-5` -> `opus-5`, `claude-haiku-4-5-20251001` -> `haiku-4-5`.
fn short_model(model: &str) -> String {
    let trimmed = model.strip_prefix("claude-").unwrap_or(model);
    match trimmed.rsplit_once('-') {
        Some((head, tail)) if tail.len() == 8 && tail.chars().all(|c| c.is_ascii_digit()) => {
            head.to_string()
        }
        _ => trimmed.to_string(),
    }
}

/// Cells taken by the marker, a space, and `parts` joined by separators.
///
/// Terminal cells, not chars: a wide name or model would otherwise overflow the row.
fn row_width(marker: &str, parts: &[(String, Rgb)]) -> usize {
    let text: usize = parts.iter().map(|(text, _)| text.width()).sum();
    marker.width() + 1 + text + SEPARATOR.width() * parts.len().saturating_sub(1)
}

/// Tokens gained per sampling interval, as bars scaled to the busiest interval shown.
///
/// The counts only grow, so plotting them directly would always draw a ramp. Growth
/// tells a busy agent from a stalled one, which draws a flat `▁▁▁`.
fn sparkline(samples: &[u64]) -> Option<String> {
    let recent = &samples[samples.len().saturating_sub(SPARK_WIDTH + 1)..];
    let gains: Vec<u64> = recent
        .windows(2)
        .map(|pair| pair[1].saturating_sub(pair[0]))
        .collect();
    let peak = gains.iter().copied().max()?;
    let bars = gains.iter().map(|&gain| {
        if gain == 0 {
            SPARK_BARS[0]
        } else {
            // 1..=7: any growth at all clears the stalled bar
            SPARK_BARS[gain.saturating_mul(7).div_ceil(peak) as usize]
        }
    });
    Some(bars.collect())
}

/// Insert `text` at `index` when the row still fits in `columns`.
fn insert_if_fits(
    parts: &mut Vec<(String, Rgb)>,
    index: usize,
    text: String,
    marker: &str,
    columns: Option<usize>,
) {
    let width = row_width(marker, parts) + SEPARATOR.width() + text.width();
    if columns.is_none_or(|columns| width <= columns) {
        parts.insert(index, (text, DIM));
    }
}

/// `now` is Unix epoch seconds, passed in so the elapsed time is deterministic under test.
fn render_row(task: &Task, columns: Option<usize>, now: u64) -> String {
    let (marker, marker_color) = status_marker(task.status.as_deref());

    // Fixed columns, in display order
    let mut parts: Vec<(String, Rgb)> = Vec::new();
    parts.push((
        task.name.clone().unwrap_or_else(|| "task".to_string()),
        WHITE,
    ));

    let mut model_text = task.model.as_deref().map(short_model).unwrap_or_default();
    if let Some(effort) = &task.effort {
        if !model_text.is_empty() {
            model_text.push(' ');
        }
        model_text.push_str(&effort.label());
    }
    if !model_text.is_empty() {
        parts.push((model_text, DIM));
    }

    if let Some(tokens) = task.token_count.filter(|&t| t > 0) {
        let mut text = fmt_tokens(tokens);
        let mut color = DIM;
        if let Some(size) = task.context_window_size
            && size > 0
        {
            let percent = tokens.saturating_mul(100) / size;
            text = format!("{text}/{} {percent}%", fmt_tokens(size));
            color = usage_colors(f64::from(u32::try_from(percent).unwrap_or(u32::MAX))).0;
        }
        parts.push((text, color));
    }

    let detail = task
        .label
        .clone()
        .or_else(|| task.description.clone())
        .filter(|d| !d.is_empty());

    // Running time, then recent activity before it, each only when it fits next to the
    // detail's minimum: what the agent is doing matters more. A finished task reports
    // when it started but not when it ended, and its samples stop growing.
    if is_running(task.status.as_deref()) {
        let reserve = detail
            .as_ref()
            .map_or(0, |_| SEPARATOR.width() + MIN_DETAIL_WIDTH);
        let room = columns.map(|columns| columns.saturating_sub(reserve));
        let tail = parts.len();
        if let Some(started) = task.start_time.map(|ms| ms / 1_000)
            && started <= now
        {
            let elapsed = fmt_duration_secs(now - started);
            insert_if_fits(&mut parts, tail, elapsed, marker, room);
        }
        if let Some(activity) = sparkline(&task.token_samples) {
            insert_if_fits(&mut parts, tail, activity, marker, room);
        }
    }

    // The detail column absorbs whatever width is left over
    if let Some(detail) = detail {
        let budget = columns
            .unwrap_or(usize::MAX)
            .saturating_sub(row_width(marker, &parts) + SEPARATOR.width());

        if budget >= MIN_DETAIL_WIDTH {
            // Detail sits right after the name
            parts.insert(1, (truncate(&detail, budget), DIM));
        }
    }

    let styled: Vec<String> = parts
        .iter()
        .map(|(text, color)| fg_styled(text, *color))
        .collect();

    format!(
        "{} {}",
        fg_styled(marker, marker_color),
        styled.join(&fg_styled(SEPARATOR, SLATE))
    )
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut input_str = String::new();
    std::io::stdin().read_to_string(&mut input_str)?;
    let input: Input = serde_json::from_str(&input_str)?;

    let now = unix_now();
    for task in &input.tasks {
        let content = render_row(task, input.columns, now);
        println!(
            "{}",
            serde_json::json!({ "id": task.id, "content": content })
        );
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

    // On failure emit nothing: Claude Code keeps the default row rendering.
    // The reason goes to stderr, which `claude --debug` logs.
    if let Err(error) = run() {
        eprintln!("claude_subagent_statusline: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use render::strip_ansi;
    use serde_json::json;

    const NOW: u64 = 1_750_000_000;

    fn parse(payload: &serde_json::Value) -> Input {
        serde_json::from_value(payload.clone()).expect("payload parses")
    }

    fn explore() -> serde_json::Value {
        json!({
            "id": "t1", "name": "Explore", "status": "running", "label": "scanning src/",
            "model": "claude-opus-5", "effort": "high",
            "contextWindowSize": 200_000, "tokenCount": 12_500,
            "startTime": (NOW - 134) * 1_000 + 999,
            "tokenSamples": [0, 0, 500, 1_500, 1_500, 1_600],
        })
    }

    #[test]
    fn short_model_drops_the_prefix_and_date_suffix() {
        assert_eq!(short_model("claude-opus-5"), "opus-5");
        assert_eq!(short_model("claude-haiku-4-5-20251001"), "haiku-4-5");
        assert_eq!(short_model("gpt-x"), "gpt-x");
    }

    #[test]
    fn row_shows_detail_model_and_context_share() {
        let input = parse(&json!({ "columns": 80, "tasks": [explore()] }));
        let row = strip_ansi(&render_row(&input.tasks[0], input.columns, NOW));
        assert_eq!(
            row,
            "\u{25cf} Explore \u{b7} scanning src/ \u{b7} opus-5 high \u{b7} 12k/200k 6% \u{b7} \u{2581}\u{2585}\u{2588}\u{2581}\u{2582} \u{b7} 2m14s"
        );
    }

    #[test]
    fn running_time_shows_only_while_the_task_runs() {
        let row = |status: &str| {
            let mut task = explore();
            task["status"] = json!(status);
            let input = parse(&json!({ "tasks": [task] }));
            strip_ansi(&render_row(&input.tasks[0], None, NOW))
        };
        assert!(row("running").ends_with(" 2m14s"), "{}", row("running"));
        assert!(row("completed").ends_with(" 6%"), "{}", row("completed"));
        assert!(row("killed").starts_with('\u{2717}'), "{}", row("killed"));

        // A start time ahead of the local clock is skew, not a duration
        let mut task = explore();
        task["startTime"] = json!((NOW + 5) * 1_000);
        task["tokenSamples"] = json!([]);
        let input = parse(&json!({ "tasks": [task] }));
        let row = strip_ansi(&render_row(&input.tasks[0], None, NOW));
        assert!(row.ends_with(" 6%"), "{row}");
    }

    #[test]
    fn detail_is_truncated_then_dropped_as_the_row_narrows() {
        let mut task = explore();
        task["label"] = json!("a long description of what the agent is doing");
        let input = parse(&json!({ "tasks": [task] }));

        let row = strip_ansi(&render_row(&input.tasks[0], Some(60), NOW));
        assert!(row.contains('\u{2026}'), "{row}");
        assert!(row.width() <= 60, "{row}");

        let row = strip_ansi(&render_row(&input.tasks[0], Some(30), NOW));
        assert!(!row.contains("long"), "{row}");
    }

    #[test]
    fn wide_characters_stay_within_the_row_budget() {
        let mut task = explore();
        task["name"] = json!("調査");
        task["label"] = json!("リポジトリ全体のソースコードを検索しています 🔍🔍🔍");
        let input = parse(&json!({ "tasks": [task] }));

        for columns in [40, 50, 60, 80] {
            let row = strip_ansi(&render_row(&input.tasks[0], Some(columns), NOW));
            assert!(
                row.width() <= columns,
                "{columns}: {row} is {} cells",
                row.width()
            );
        }
    }

    #[test]
    fn sparkline_plots_recent_growth_scaled_to_the_busiest_interval() {
        // Gains 0, 500, 1000, 0, 100: any growth clears the bottom bar
        assert_eq!(
            sparkline(&[0, 0, 500, 1_500, 1_500, 1_600]).as_deref(),
            Some("\u{2581}\u{2585}\u{2588}\u{2581}\u{2582}")
        );
        // Stalled
        assert_eq!(
            sparkline(&[900, 900, 900]).as_deref(),
            Some("\u{2581}\u{2581}")
        );
        // A count that drops, after a compaction, reads as no growth
        assert_eq!(
            sparkline(&[900, 100, 600]).as_deref(),
            Some("\u{2581}\u{2588}")
        );
        // Only the most recent intervals
        let samples: Vec<u64> = (0..16).map(|i| i * 100).collect();
        assert_eq!(
            sparkline(&samples).map(|s| s.chars().count()),
            Some(SPARK_WIDTH)
        );
        assert_eq!(sparkline(&[42]), None);
        assert_eq!(sparkline(&[]), None);
    }

    #[test]
    fn effort_can_be_a_token_budget() {
        let mut task = explore();
        task["effort"] = json!(16_000);
        let input = parse(&json!({ "tasks": [task] }));
        let row = strip_ansi(&render_row(&input.tasks[0], None, NOW));
        assert!(row.contains("opus-5 16k"), "{row}");
    }

    #[test]
    fn a_malformed_task_is_skipped_and_bad_fields_are_dropped() {
        let input = parse(&json!({
            "columns": "wide",
            "tasks": [
                explore(),
                { "name": "no id" },
                { "id": "t3", "name": "Plan", "tokenCount": "lots" },
            ],
        }));
        assert_eq!(input.columns, None);
        let ids: Vec<&str> = input.tasks.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, ["t1", "t3"]);
        assert_eq!(input.tasks[1].token_count, None);
    }

    #[test]
    fn tasks_that_are_not_an_array_render_nothing() {
        assert!(parse(&json!({ "tasks": { "id": "t1" } })).tasks.is_empty());
    }
}
