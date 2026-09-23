# Claude Statusline

A powerline-style statusline renderer for [Claude Code](https://code.claude.com/docs/en/statusline). It reads session data as JSON from stdin and outputs ANSI-colored, powerline-styled text.

```
 claude_statusline   master  ⧉ my-feature ← master  PR #1234 approved            session-name
 Opus ⚡ ✻ high  security-reviewer  +156/-23  37% 74k/200k ↺61k  cache 91% 42m   $0.42  12m34s  5h 24% (2h13m)  7d 41% (3d5h)
```

The output is two rows: the first says *where* you are working, the second says *how* the session is going. Every section is hidden when its data is absent, so a fresh session in a plain directory renders just the directory and the model.

Each row is split into a left group and a right group pinned to the right edge, so spend and rate limits keep a fixed screen position instead of drifting as the left side grows.

## Sections

Each table entry is one section of the example above, listed left to right.

### Row 1 — location

| Section | Example | Color | Description |
|---------|---------|-------|-------------|
| Directory | `claude_statusline` | Blue | Current directory name. Clickable (OSC 8) link to the remote repo when `workspace.repo` is present |
| Git branch | ` master` | Green | Branch you're on: `worktree.branch` when available, otherwise `git branch --show-current` in the workspace directory |
| Worktree | `⧉ my-feature ← master` | Teal | Worktree name (`--worktree` session or linked git worktree), then `←` and the branch it was created from |
| Pull request | `PR #1234 approved` | State-colored | Open PR for the current branch and its review state, clickable. `MR !1234` for a GitLab merge request. Green approved / red changes requested / yellow pending / slate draft |
| Session name | `session-name` | Slate | **Right-aligned.** Only when set with `--name`, `/rename`, or an AI-generated title |

### Row 2 — session state

| Section | Example | Color | Description |
|---------|---------|-------|-------------|
| Model | `Opus ⚡ ✻ high` | Gray | Model name, then `⚡` when fast mode is on, `✻` when extended thinking is on, and the reasoning effort level |
| Agent | `security-reviewer` | Orange | Agent the session runs as (`--agent` or agent settings) |
| Lines changed | `+156/-23` | Olive | Lines added and removed during the session |
| Context | `37% 74k/200k ↺61k` | Green / Yellow / Red | How full the context window is, tokens used out of its size, and `↺` tokens the last request read from the prompt cache. A `200k+` marker appears past 200k tokens |
| Prompt cache | `cache 91% 42m` | Cyan / Yellow / Slate | Warm: share of this session's input served from the cache, and minutes until it expires (seconds in the last minute); yellow in the last fifth of its TTL. Needs `refreshInterval` to count down while idle. Cold: `cache cold ↻45k`, the tokens your next message re-caches. Hidden when caching isn't observed |
| Cost | `$0.42` | Purple | **Right-aligned.** Estimated session cost in USD (hidden below $0.01) |
| Duration | `12m34s` | Indigo | **Right-aligned.** How long the session has been running |
| 5-hour limit | `5h 24% (2h13m)` | Green / Yellow / Red | **Right-aligned.** Share of the 5-hour subscription limit used, and time until it resets |
| 7-day limit | `7d 41% (3d5h)` | Green / Yellow / Red | **Right-aligned.** The same for the 7-day limit |
| Spend limit | `spend 63% (12d0h)` | Green / Yellow / Red | **Right-aligned.** Not in the example: appears only behind a Claude apps gateway that sets a spend limit. Share used, which can pass 100%, and time until it resets |

Threshold colors for context and rate limits:
- **Green** -- 0-59%
- **Yellow** -- 60-80%
- **Red** -- above 80%

### Right alignment

Claude Code captures the script's stdout, so `tput cols` cannot see the terminal. It exports
`COLUMNS` and `LINES` instead (v2.1.153+), and the renderer pads between the two groups to push
the right one against the edge. Widths are measured in terminal cells with `unicode-width`, not
bytes or chars, so the powerline glyphs and emoji markers line up.

It degrades rather than wraps. When `COLUMNS` is unset, or the two groups would collide on a
narrow terminal, everything renders as one continuous powerline with no padding.

`COLUMNS` is the full terminal width, but Claude Code draws the status line inside its own chrome
and truncates whatever overflows, so five cells are held back from the right edge. Change that with
`CLAUDE_STATUSLINE_RIGHT_MARGIN` -- raise it if the right group still gets clipped, lower it if the
gap looks too wide:

```json
{ "env": { "CLAUDE_STATUSLINE_RIGHT_MARGIN": "7" } }
```

To move a section between sides, move its `Section::new(...)` push between the `sections` and
`right` vectors in `location_sections` / `session_sections` in `src/main.rs`.

## Requirements

- Rust 1.88+
- A terminal with true color (24-bit) support
- A font with powerline glyphs (e.g. Nerd Font)
- Clickable PR and repo links need a terminal with OSC 8 support (Ghostty, iTerm2, Kitty, WezTerm)

## Build

```sh
cargo build --release
```

Two binaries are produced in `target/release/`:

- `claude_statusline` -- the main status line
- `claude_subagent_statusline` -- one row per subagent in the agent panel

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/gdarmont/claude_statusline/master/install.sh | bash
```

Detects your platform, downloads the matching zip from the latest release, verifies it against the
published `SHA256SUMS`, backs up any existing `claude_statusline` to `claude_statusline.bak` on
first run, and installs both binaries into `~/.claude/`. Prebuilt binaries cover x86_64 Linux
(static musl) and both Apple Silicon and Intel macOS; anything else needs a source build.

| Variable | Default | Purpose |
|----------|---------|---------|
| `CLAUDE_DIR` | `~/.claude` | Install directory |
| `VERSION` | latest release | Pin to a specific tag, e.g. `v0.9.0` |

```sh
curl -fsSL https://raw.githubusercontent.com/gdarmont/claude_statusline/master/install.sh \
  | VERSION=v0.9.0 CLAUDE_DIR=/opt/claude bash
```

### From source

```sh
./dev-install.sh
```

Builds both binaries in release mode, installs them the same way, and smoke-tests what landed. Or
by hand:

```sh
cargo build --release
cp target/release/claude_statusline target/release/claude_subagent_statusline ~/.claude/
```

## Releasing

Releases are built by [`.github/workflows/release.yml`](.github/workflows/release.yml) when a `v*`
tag is pushed. Bump `version` in `Cargo.toml` first -- the workflow refuses to build when the tag
and the crate version disagree.

```sh
cargo build --release   # refresh Cargo.lock, which CI installs with --locked
git commit -am "Release v0.9.0"
git tag -a v0.9.0 -m "v0.9.0"
git push origin master --follow-tags
```

Each platform job zips both binaries together, and the release job publishes every zip plus a
`SHA256SUMS` file with auto-generated release notes.

## Configure Claude Code

In `~/.claude/settings.json`:

```json
{
  "statusLine": {
    "type": "command",
    "command": "~/.claude/claude_statusline",
    "padding": 0,
    "refreshInterval": 15
  },
  "subagentStatusLine": {
    "type": "command",
    "command": "~/.claude/claude_subagent_statusline"
  }
}
```

Restart Claude Code for the change to take effect.

`refreshInterval` re-runs the status line every 15 seconds on top of Claude Code's own triggers,
which fire on events such as a new message or `/compact`. Without it, nothing redraws while you're
idle: the prompt cache countdown freezes at its last value, and the cache never turns yellow as it
nears expiry, which is exactly when you'd want to see it. The binary finishes in a few
milliseconds, so the cost is negligible.

## Input format

Claude Code pipes a JSON object to stdin on every refresh. Every field below is optional except `workspace.current_dir` and `model.display_name`:

```json
{
  "session_name": "my-session",
  "model": { "display_name": "Opus" },
  "workspace": {
    "current_dir": "/home/user/projects/myproject",
    "git_worktree": "feature-xyz",
    "repo": { "host": "github.com", "owner": "anthropics", "name": "claude-code" }
  },
  "cost": {
    "total_cost_usd": 0.42,
    "total_duration_ms": 754000,
    "total_lines_added": 156,
    "total_lines_removed": 23
  },
  "context_window": {
    "total_input_tokens": 74500,
    "context_window_size": 200000,
    "used_percentage": 37.2,
    "current_usage": { "cache_read_input_tokens": 61000 }
  },
  "exceeds_200k_tokens": false,
  "fast_mode": true,
  "effort": { "level": "high" },
  "thinking": { "enabled": true },
  "rate_limits": {
    "five_hour": { "used_percentage": 23.5, "resets_at": 1738425600 },
    "seven_day": { "used_percentage": 41.2, "resets_at": 1738857600 },
    "spend_limit": { "used_percentage": 62.8, "resets_at": 1740787200 }
  },
  "prompt_cache": {
    "warm": true,
    "caching_observed": true,
    "ttl": "1h",
    "expires_at": 1738429200,
    "hit_ratio": 0.91,
    "recache_tokens_if_cold": 45000
  },
  "agent": { "name": "security-reviewer" },
  "pr": { "number": 1234, "url": "https://github.com/o/r/pull/1234", "review_state": "approved" },
  "worktree": { "name": "my-feature", "branch": "worktree-my-feature", "original_branch": "main" }
}
```

Notes on availability:

- `rate_limits` appears for Claude.ai Pro/Max subscribers after the first API response. `spend_limit` appears only behind a Claude apps gateway that sets one (v2.1.251+)
- `prompt_cache` appears after the main conversation's first API response (v2.1.251+). `expires_at` is `null` while the cache is cold
- `pr` appears only while an open PR or GitLab merge request exists for the branch. `kind` is `mr` for a merge request and absent for GitHub (v2.1.234+)
- `effort` appears only on models supporting the reasoning effort parameter
- `context_window.current_usage` is `null` before the first API call and after `/compact`

### Subagent rows

`claude_subagent_statusline` receives all visible subagent rows at once and writes one
`{"id": ..., "content": ...}` line per row:

```json
{
  "columns": 80,
  "tasks": [
    {
      "id": "t1", "name": "Explore", "status": "running", "label": "scanning src/",
      "description": "Search the repo", "model": "claude-opus-5", "effort": "high",
      "contextWindowSize": 200000, "tokenCount": 12500
    }
  ]
}
```

Rows render as `● Explore · scanning src/ · opus-5 high · 12k/200k 6%`, with the detail column
truncated to fit `columns` and dropped entirely when there is no room for it. `model` and
`contextWindowSize` need Claude Code v2.1.205+; `effort` needs v2.1.214+.

## Testing

```sh
cargo test
```

Unit tests sit next to the code in each binary. `tests/cli.rs` runs the built binaries end to
end, JSON on stdin, the way Claude Code does. CI runs formatting, clippy with warnings as errors,
and the tests on Linux and macOS, plus a build on the `rust-version` from `Cargo.toml`.

A field whose type changes in a new Claude Code release is treated as absent, so only its section
disappears. If the whole line shows `[statusline]` instead, the payload itself couldn't be parsed;
the reason is on stderr, which `claude --debug` logs.

To try a payload by hand:

```sh
echo '{"workspace":{"current_dir":"/tmp/demo"},"model":{"display_name":"Opus"}}' \
  | ./target/release/claude_statusline

echo '{"columns":80,"tasks":[{"id":"t1","name":"Explore","status":"running","tokenCount":12500,"contextWindowSize":200000}]}' \
  | ./target/release/claude_subagent_statusline
```
