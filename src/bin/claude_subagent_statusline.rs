//! Renders one row per visible subagent for the `subagentStatusLine` setting.
//!
//! Reads every visible task as a single JSON object on stdin and writes one
//! `{"id": ..., "content": ...}` line per row it wants to override.

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
    columns: Option<usize>,
    #[serde(default)]
    tasks: Vec<Task>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Task {
    id: String,
    name: Option<String>,
    status: Option<String>,
    description: Option<String>,
    label: Option<String>,
    /// Resolved model ID (Claude Code v2.1.205+).
    model: Option<String>,
    /// Effort level string, or a numeric token budget (v2.1.214+).
    effort: Option<Effort>,
    context_window_size: Option<u64>,
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
    let _ = run();
}
