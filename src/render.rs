//! Shared rendering primitives: colors, powerline sections, formatting helpers.
//!
//! Included by both binaries; each one uses a subset of what lives here.
#![allow(dead_code)]

use std::fmt::Write as _;
use unicode_width::{UnicodeWidthChar as _, UnicodeWidthStr as _};

const LEFT_ROUND: &str = "\u{e0b6}";
const RIGHT_ARROW: &str = "\u{e0b0}";
/// Thin chevron, used between two sections that share a background color.
const RIGHT_THIN: &str = "\u{e0b1}";
const RIGHT_ROUND: &str = "\u{e0b4}";

pub const RESET: &str = "\x1b[0m";

/// Usage above this percentage is rendered as a warning.
pub const WARNING_THRESHOLD: f64 = 59.0;
/// Usage above this percentage is rendered as danger.
pub const DANGER_THRESHOLD: f64 = 80.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    fn bg_ansi(self) -> String {
        format!("\x1b[48;2;{};{};{}m", self.0, self.1, self.2)
    }

    fn fg_ansi(self) -> String {
        format!("\x1b[38;2;{};{};{}m", self.0, self.1, self.2)
    }
}

pub const BLACK: Rgb = Rgb(0, 0, 0);
pub const WHITE: Rgb = Rgb(255, 255, 255);
pub const BLUE: Rgb = Rgb(52, 86, 164);
pub const GREEN: Rgb = Rgb(70, 107, 62);
pub const GRAY: Rgb = Rgb(68, 68, 68);
pub const PURPLE: Rgb = Rgb(128, 90, 140);
pub const YELLOW: Rgb = Rgb(204, 153, 0);
pub const RED: Rgb = Rgb(226, 0, 0);
pub const TEAL: Rgb = Rgb(38, 108, 118);
pub const SLATE: Rgb = Rgb(84, 88, 106);
pub const ORANGE: Rgb = Rgb(158, 88, 38);
pub const OLIVE: Rgb = Rgb(58, 84, 52);
pub const INDIGO: Rgb = Rgb(66, 66, 96);
pub const CYAN: Rgb = Rgb(34, 110, 140);

/// Priority of a section that is never dropped from a row too wide for the terminal.
pub const ESSENTIAL: u8 = u8::MAX;

pub struct Section {
    pub text: String,
    pub bg: Rgb,
    pub fg: Rgb,
    /// When set, the whole cell becomes an OSC 8 hyperlink.
    pub url: Option<String>,
    /// A row too wide for the terminal drops its lowest priority sections first.
    pub priority: u8,
}

impl Section {
    pub fn new(text: impl Into<String>, bg: Rgb, fg: Rgb) -> Self {
        Self {
            text: text.into(),
            bg,
            fg,
            url: None,
            priority: ESSENTIAL,
        }
    }

    pub fn link(mut self, url: Option<String>) -> Self {
        self.url = url;
        self
    }

    pub fn priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }
}

pub fn ansi_styled(text: &str, bg: Rgb, fg: Rgb) -> String {
    format!("{RESET}{}{}{text}{RESET}", bg.bg_ansi(), fg.fg_ansi())
}

/// Foreground-only styling, for contexts that keep the terminal background.
pub fn fg_styled(text: &str, fg: Rgb) -> String {
    format!("{}{text}{RESET}", fg.fg_ansi())
}

/// Wrap text in an OSC 8 hyperlink. Terminals without support render the text as-is.
pub fn hyperlink(text: &str, url: &str) -> String {
    format!("\x1b]8;;{url}\x07{text}\x1b]8;;\x07")
}

pub fn format_sections(sections: &[Section]) -> String {
    let mut out = String::new();

    let Some(first) = sections.first() else {
        return out;
    };

    // Leading round cap
    let _ = write!(out, "{}", cap(LEFT_ROUND, first.bg));

    for (i, section) in sections.iter().enumerate() {
        if i > 0 {
            let previous = &sections[i - 1];
            // A solid arrow is invisible between two sections of the same color,
            // so fall back to a thin chevron drawn in the foreground color.
            let separator = if previous.bg == section.bg {
                ansi_styled(RIGHT_THIN, section.bg, section.fg)
            } else {
                ansi_styled(RIGHT_ARROW, section.bg, previous.bg)
            };
            let _ = write!(out, "{separator}");
        }
        let padded = format!(" {} ", section.text);
        let cell = match &section.url {
            Some(url) => hyperlink(&padded, url),
            None => padded,
        };
        let _ = write!(out, "{}", ansi_styled(&cell, section.bg, section.fg));
    }

    // Trailing round cap
    if let Some(last) = sections.last() {
        let _ = write!(out, "{}", cap(RIGHT_ROUND, last.bg));
    }

    out
}

/// A round cap in the section's color, on the terminal's own background so it
/// blends into any theme, the way the gap between the two groups does.
fn cap(glyph: &str, color: Rgb) -> String {
    format!("{RESET}{}", fg_styled(glyph, color))
}

/// Cells a section group occupies once rendered: two round caps, one separator
/// between each pair of sections, and every text padded with a space either side.
pub fn group_width(sections: &[Section]) -> usize {
    if sections.is_empty() {
        return 0;
    }
    let glyphs = 2 + (sections.len() - 1);
    let text: usize = sections.iter().map(|s| s.text.width() + 2).sum();
    glyphs + text
}

/// Terminal width. Claude Code captures our stdout, so `tput cols` cannot see
/// the terminal; it exports `COLUMNS` for us instead (v2.1.153+).
pub fn terminal_columns() -> Option<usize> {
    std::env::var("COLUMNS")
        .ok()?
        .trim()
        .parse::<usize>()
        .ok()
        .filter(|&columns| columns > 0)
}

/// Cells kept free at the right edge so a full-width row never wraps.
///
/// `COLUMNS` is the terminal width, but Claude Code renders the status line
/// inside its own chrome and truncates what overflows, so the usable width is
/// smaller. Override with `CLAUDE_STATUSLINE_RIGHT_MARGIN`.
const DEFAULT_RIGHT_MARGIN: usize = 5;

fn right_margin() -> usize {
    std::env::var("CLAUDE_STATUSLINE_RIGHT_MARGIN")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(DEFAULT_RIGHT_MARGIN)
}

/// Render `left` flush left and `right` flush right on a single row, sized from the environment.
pub fn format_row(left: Vec<Section>, right: Vec<Section>) -> String {
    let usable = terminal_columns().map(|columns| columns.saturating_sub(right_margin()));
    format_row_within(left, right, usable)
}

/// Render `left` flush left and `right` flush right within `usable` cells.
///
/// When the two groups would collide, they join into one continuous powerline.
/// When even that is too wide, the lowest priority sections are dropped until it
/// fits, rather than leaving Claude Code to clip whatever reaches the right edge.
/// An unknown width gets the continuous powerline, whole.
pub fn format_row_within(
    mut left: Vec<Section>,
    mut right: Vec<Section>,
    usable: Option<usize>,
) -> String {
    if let Some(usable) = usable {
        loop {
            let total = group_width(&left) + group_width(&right);
            // Strictly less, so there is always at least one cell of daylight.
            if !right.is_empty() && total < usable {
                return format!(
                    "{}{}{}",
                    format_sections(&left),
                    " ".repeat(usable - total),
                    format_sections(&right)
                );
            }
            if joined_width(&left, &right) <= usable || !drop_lowest(&mut left, &mut right) {
                break;
            }
        }
    }

    left.extend(right);
    format_sections(&left)
}

/// Cells of `left` and `right` drawn as one continuous powerline: the two inner
/// caps become a single separator.
fn joined_width(left: &[Section], right: &[Section]) -> usize {
    if left.is_empty() || right.is_empty() {
        group_width(left) + group_width(right)
    } else {
        group_width(left) + group_width(right) - 1
    }
}

/// Remove the lowest priority section of either group. False when only essential
/// sections are left.
fn drop_lowest(left: &mut Vec<Section>, right: &mut Vec<Section>) -> bool {
    let in_left = left.iter().enumerate().map(|(i, s)| (s.priority, false, i));
    let in_right = right.iter().enumerate().map(|(i, s)| (s.priority, true, i));
    let Some((_, right_group, index)) = in_left
        .chain(in_right)
        .filter(|&(priority, ..)| priority < ESSENTIAL)
        .min_by_key(|&(priority, ..)| priority)
    else {
        return false;
    };
    if right_group {
        right.remove(index);
    } else {
        left.remove(index);
    }
    true
}

/// Green / yellow / red pair for a usage percentage.
pub fn usage_colors(percent: f64) -> (Rgb, Rgb) {
    if percent > DANGER_THRESHOLD {
        (RED, WHITE)
    } else if percent > WARNING_THRESHOLD {
        (YELLOW, BLACK)
    } else {
        (GREEN, WHITE)
    }
}

/// Compact token count: `950`, `74k`, `1.2M`.
pub fn fmt_tokens(tokens: u64) -> String {
    if tokens >= 1_000_000 {
        let whole = tokens / 1_000_000;
        let tenth = (tokens % 1_000_000) / 100_000;
        format!("{whole}.{tenth}M")
    } else if tokens >= 1_000 {
        format!("{}k", tokens / 1_000)
    } else {
        tokens.to_string()
    }
}

/// Compact duration: `45s`, `12m34s`, `2h13m`, `3d4h`.
pub fn fmt_duration_secs(secs: u64) -> String {
    let days = secs / 86_400;
    let hours = (secs % 86_400) / 3_600;
    let minutes = (secs % 3_600) / 60;
    let seconds = secs % 60;

    if days > 0 {
        format!("{days}d{hours}h")
    } else if hours > 0 {
        format!("{hours}h{minutes:02}m")
    } else if minutes > 0 {
        format!("{minutes}m{seconds:02}s")
    } else {
        format!("{seconds}s")
    }
}

pub fn fmt_duration_ms(ms: u64) -> String {
    fmt_duration_secs(ms / 1_000)
}

pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Truncate to at most `max` terminal cells, appending an ellipsis when shortened.
///
/// Wide characters (CJK, most emoji) take two cells, so counting chars would overflow.
pub fn truncate(text: &str, max: usize) -> String {
    if text.width() <= max {
        return text.to_string();
    }
    if max <= 1 {
        return "\u{2026}".to_string();
    }
    // Leave one cell for the ellipsis
    let budget = max - 1;
    let mut out = String::new();
    let mut used = 0;
    for c in text.chars() {
        let cells = c.width().unwrap_or(0);
        if used + cells > budget {
            break;
        }
        used += cells;
        out.push(c);
    }
    out.push('\u{2026}');
    out
}

/// Drop ANSI styling and OSC 8 hyperlinks, leaving the text a terminal would show.
#[cfg(test)]
pub fn strip_ansi(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\x1b' {
            out.push(c);
            continue;
        }
        match chars.next() {
            // CSI: parameters up to a final byte such as `m`
            Some('[') => {
                for c in chars.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
            // OSC, used for hyperlinks: up to the BEL terminator
            Some(']') => {
                for c in chars.by_ref() {
                    if c == '\x07' {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    out
}
