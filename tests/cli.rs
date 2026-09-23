//! End-to-end: run the built binaries the way Claude Code does, JSON on stdin.

use std::io::Write as _;
use std::process::{Command, Stdio};
use unicode_width::UnicodeWidthStr as _;

const STATUSLINE: &str = env!("CARGO_BIN_EXE_claude_statusline");
const SUBAGENT: &str = env!("CARGO_BIN_EXE_claude_subagent_statusline");

struct Output {
    stdout: String,
    stderr: String,
}

fn run(bin: &str, stdin: &str, env: &[(&str, &str)]) -> Output {
    let mut child = Command::new(bin)
        .env_remove("COLUMNS")
        .env_remove("CLAUDE_STATUSLINE_RIGHT_MARGIN")
        .envs(env.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("binary starts");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(stdin.as_bytes())
        .expect("stdin accepts the payload");
    let output = child.wait_with_output().expect("binary exits");
    assert!(
        output.status.success(),
        "a status line must never exit non-zero"
    );
    Output {
        stdout: String::from_utf8(output.stdout).expect("utf-8 stdout"),
        stderr: String::from_utf8(output.stderr).expect("utf-8 stderr"),
    }
}

/// Drop ANSI styling and OSC 8 hyperlinks, leaving the text a terminal would show.
fn strip_ansi(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\x1b' {
            out.push(c);
            continue;
        }
        let terminator = match chars.next() {
            Some('[') => |c: char| ('@'..='~').contains(&c),
            Some(']') => |c: char| c == '\x07',
            _ => continue,
        };
        for c in chars.by_ref() {
            if terminator(c) {
                break;
            }
        }
    }
    out
}

fn visible_lines(stdout: &str) -> Vec<String> {
    stdout.lines().map(strip_ansi).collect()
}

const MINIMAL: &str =
    r#"{"workspace":{"current_dir":"/nonexistent/demo"},"model":{"display_name":"Opus"}}"#;

#[test]
fn minimal_payload_renders_directory_and_model() {
    let out = run(STATUSLINE, MINIMAL, &[]);
    let lines = visible_lines(&out.stdout);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines[0].contains(" demo "), "{lines:?}");
    assert!(lines[1].contains(" Opus "), "{lines:?}");
    assert!(out.stderr.is_empty(), "{}", out.stderr);
}

#[test]
fn unparseable_input_shows_a_placeholder_and_explains_on_stderr() {
    for payload in ["not json", r#"{"workspace":{"current_dir":"/tmp"}}"#] {
        let out = run(STATUSLINE, payload, &[]);
        assert_eq!(out.stdout, "[statusline]\n");
        assert!(
            out.stderr.starts_with("claude_statusline: "),
            "{}",
            out.stderr
        );
    }
}

#[test]
fn schema_drift_hides_only_the_affected_sections() {
    let payload = r#"{
        "workspace": {"current_dir": "/nonexistent/demo"},
        "model": {"display_name": "Opus"},
        "cost": {"total_cost_usd": "0.42", "total_lines_added": 3, "total_lines_removed": 1},
        "context_window": {"used_percentage": 37},
        "rate_limits": {"five_hour": "soon"},
        "some_future_field": {"nested": [1, 2, 3]}
    }"#;
    let out = run(STATUSLINE, payload, &[]);
    let lines = visible_lines(&out.stdout);
    assert!(lines[1].contains("Opus"), "{lines:?}");
    assert!(lines[1].contains("+3/-1"), "{lines:?}");
    assert!(lines[1].contains("37%"), "{lines:?}");
    assert!(!lines[1].contains('$'), "{lines:?}");
    assert!(out.stderr.is_empty(), "{}", out.stderr);
}

#[test]
fn right_group_ends_at_columns_minus_the_margin() {
    let payload = r#"{
        "workspace": {"current_dir": "/nonexistent/demo"},
        "model": {"display_name": "Opus"},
        "cost": {"total_cost_usd": 0.42}
    }"#;
    let session_row =
        |env: &[(&str, &str)]| visible_lines(&run(STATUSLINE, payload, env).stdout)[1].clone();

    assert_eq!(session_row(&[("COLUMNS", "120")]).width(), 115);
    let no_margin = [("COLUMNS", "120"), ("CLAUDE_STATUSLINE_RIGHT_MARGIN", "0")];
    assert_eq!(session_row(&no_margin).width(), 120);
    // Width unknown: no padding at all
    assert!(!session_row(&[]).contains("  "));
}

#[test]
fn subagent_writes_one_json_line_per_parseable_task() {
    let payload = r#"{
        "columns": 80,
        "tasks": [
            {"id": "t1", "name": "Explore", "status": "running", "tokenCount": 12500, "contextWindowSize": 200000},
            {"name": "missing id"},
            {"id": "t2", "name": "Plan", "status": "completed", "tokenCount": "many"}
        ]
    }"#;
    let out = run(SUBAGENT, payload, &[]);
    let rows: Vec<serde_json::Value> = out
        .stdout
        .lines()
        .map(|line| serde_json::from_str(line).expect("each line is JSON"))
        .collect();
    let ids: Vec<&str> = rows.iter().filter_map(|row| row["id"].as_str()).collect();
    assert_eq!(ids, ["t1", "t2"]);
    let first = strip_ansi(rows[0]["content"].as_str().expect("content"));
    assert!(
        first.contains("Explore") && first.contains("12k/200k 6%"),
        "{first}"
    );
}

#[test]
fn version_flag_prints_the_crate_version_without_reading_stdin() {
    for (bin, name) in [
        (STATUSLINE, "claude_statusline"),
        (SUBAGENT, "claude_subagent_statusline"),
    ] {
        for flag in ["--version", "-V"] {
            // Empty stdin: a binary that rendered instead would print `[statusline]` or nothing
            let output = Command::new(bin)
                .arg(flag)
                .stdin(Stdio::null())
                .output()
                .expect("binary runs");
            assert!(output.status.success());
            assert_eq!(
                String::from_utf8_lossy(&output.stdout),
                format!("{name} {}\n", env!("CARGO_PKG_VERSION"))
            );
        }
    }
}

#[test]
fn subagent_emits_nothing_on_bad_input_so_defaults_stay() {
    let out = run(SUBAGENT, "not json", &[]);
    assert!(out.stdout.is_empty());
    assert!(
        out.stderr.starts_with("claude_subagent_statusline: "),
        "{}",
        out.stderr
    );
}
