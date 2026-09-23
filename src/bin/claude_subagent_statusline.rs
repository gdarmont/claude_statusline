//! Renders one row per visible subagent for the `subagentStatusLine` setting.
//!
//! Reads every visible task as a single JSON object on stdin and writes one
//! `{"id": ..., "content": ...}` line per row it wants to override.

#[path = "../lenient.rs"]
mod lenient;
#[path = "../render.rs"]
mod render;

use render::{
    BLUE, GREEN, RED, Rgb, SLATE, WHITE, YELLOW, fg_styled, fmt_tokens, truncate, usage_colors,
};
use serde::Deserialize;
use std::io::Read;

const DIM: Rgb = Rgb(120, 124, 138);
const SEPARATOR: &str = " \u{b7} ";
/// Below this the detail column is dropped rather than truncated to noise.
const MIN_DETAIL_WIDTH: usize = 8;

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

/// Marker glyph and color for a task status.
fn status_marker(status: Option<&str>) -> (&'static str, Rgb) {
    match status {
        Some("running" | "in_progress" | "active") => ("\u{25cf}", GREEN),
        Some("completed" | "done" | "success") => ("\u{2713}", BLUE),
        Some("failed" | "error" | "cancelled") => ("\u{2717}", RED),
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

fn render_row(task: &Task, columns: Option<usize>) -> String {
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

    // The detail column absorbs whatever width is left over
    let detail = task
        .label
        .clone()
        .or_else(|| task.description.clone())
        .filter(|d| !d.is_empty());

    if let Some(detail) = detail {
        let fixed: usize = parts.iter().map(|(text, _)| text.chars().count()).sum();
        // marker + space, plus a separator before every part after the name
        let overhead = 2 + SEPARATOR.chars().count() * parts.len();
        let budget = columns
            .unwrap_or(usize::MAX)
            .saturating_sub(fixed + overhead);

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

    for task in &input.tasks {
        let content = render_row(task, input.columns);
        println!(
            "{}",
            serde_json::json!({ "id": task.id, "content": content })
        );
    }

    Ok(())
}

fn main() {
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

    fn parse(payload: &serde_json::Value) -> Input {
        serde_json::from_value(payload.clone()).expect("payload parses")
    }

    fn explore() -> serde_json::Value {
        json!({
            "id": "t1", "name": "Explore", "status": "running", "label": "scanning src/",
            "model": "claude-opus-5", "effort": "high",
            "contextWindowSize": 200_000, "tokenCount": 12_500,
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
        let row = strip_ansi(&render_row(&input.tasks[0], input.columns));
        assert_eq!(
            row,
            "\u{25cf} Explore \u{b7} scanning src/ \u{b7} opus-5 high \u{b7} 12k/200k 6%"
        );
    }

    #[test]
    fn detail_is_truncated_then_dropped_as_the_row_narrows() {
        let mut task = explore();
        task["label"] = json!("a long description of what the agent is doing");
        let input = parse(&json!({ "tasks": [task] }));

        let row = strip_ansi(&render_row(&input.tasks[0], Some(60)));
        assert!(row.contains('\u{2026}'), "{row}");
        assert!(row.chars().count() <= 60, "{row}");

        let row = strip_ansi(&render_row(&input.tasks[0], Some(30)));
        assert!(!row.contains("long"), "{row}");
    }

    #[test]
    fn effort_can_be_a_token_budget() {
        let mut task = explore();
        task["effort"] = json!(16_000);
        let input = parse(&json!({ "tasks": [task] }));
        let row = strip_ansi(&render_row(&input.tasks[0], None));
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
