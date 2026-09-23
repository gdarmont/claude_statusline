# Claude Statusline

A powerline-style statusline renderer for [Claude Code](https://code.claude.com/docs/en/statusline). It reads session data as JSON from stdin and outputs ANSI-colored, powerline-styled text.

```
 claude_statusline   master  ⧉ my-feature ← master  PR #1234 approved            session-name
 Opus ⚡ ✻ high  security-reviewer  +156/-23  37% 74k/200k ↺61k  cache 91% 42m00s   $0.42  12m34s  5h 24% (2h13m)  7d 41% (3d5h)
```

Reading the example, left to right:

**Row 1: where you are**

- `claude_statusline`: the current directory. Click it to open the repository.
- ` master`: the git branch you're on.
- `⧉ my-feature ← master`: you're in the worktree `my-feature`, which was created from `master`.
- `PR #1234 approved`: the open pull request for this branch and its review status. Click it to open the PR. GitLab merge requests show as `MR !1234`.
- `session-name`: the session's name, from `--name`, `/rename` or an AI-generated title.

**Row 2: how the session is going**

- `Opus`: the model.
- `⚡`: fast mode is on.
- `✻`: extended thinking is on.
- `high`: the reasoning effort level.
- `security-reviewer`: the agent the session runs as.
- `+156/-23`: lines added and removed during the session.
- `37%`: how full the context window is.
- `74k/200k`: tokens in the context window, out of its size.
- `↺61k`: tokens the last request read from the prompt cache.
- `cache 91%`: the prompt cache is warm, and 91% of input tokens this session were served from it.
- `42m00s`: time before the cache expires. Once it expires, the segment reads `cache cold ↻45k`, the number of tokens your next message has to re-cache.
- `$0.42`: estimated session cost.
- `12m34s`: how long the session has been running.
- `5h 24% (2h13m)`: 24% of the 5-hour rate limit is used, and it resets in 2h13m.
- `7d 41% (3d5h)`: the same for the 7-day limit.
- `spend 63% (12d0h)`: not in the example. It appears only behind a Claude apps gateway that sets a spend limit, showing the share of the limit used and time until it resets.

The output is two rows: the first says *where* you are working, the second says *how* the session is going. Every section is hidden when its data is absent, so a fresh session in a plain directory renders just the directory and the model.

Each row is split into a left group and a right group pinned to the right edge, so spend and rate limits keep a fixed screen position instead of drifting as the left side grows.

## Sections

### Row 1 — location

| Section | Color | Description |
|---------|-------|-------------|
| Directory | Blue | Current directory name. Clickable (OSC 8) link to the remote repo when `workspace.repo` is present |
| Git branch | Green | `worktree.branch` when available, otherwise `git branch --show-current` in the workspace directory |
| Worktree | Teal | `--worktree` session or linked git worktree, with the branch it came from |
| Pull request | State-colored | Open PR for the current branch, clickable. `MR !n` for a GitLab merge request. Green approved / red changes requested / yellow pending / slate draft |
| Session name | Slate | **Right-aligned.** Only when set with `--name`, `/rename`, or an AI-generated title |

### Row 2 — session state

| Section | Color | Description |
|---------|-------|-------------|
| Model | Gray | Model name, plus `⚡` fast mode, `✻` thinking, and the reasoning effort level |
| Agent | Orange | Active agent name (`--agent` or agent settings) |
| Lines changed | Olive | `+added/-removed` for the session |
| Context | Green / Yellow / Red | Usage percentage, `used/total` tokens, `↺` cache reads, `200k+` marker |
| Prompt cache | Cyan / Yellow / Slate | Warm: session hit ratio and time until the cache expires, yellow in the last fifth of its TTL. Cold: `↻` tokens the next request re-caches. Hidden when caching isn't observed |
| Cost | Purple | **Right-aligned.** Session cost in USD (hidden below $0.01) |
| Duration | Indigo | **Right-aligned.** Wall-clock session time |
| Rate limits | Green / Yellow / Red | **Right-aligned.** 5-hour and 7-day subscription usage, plus the gateway `spend` limit, each with time until reset |

Threshold colors for context and rate limits (the spend limit can go past 100%):
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

- Rust 1.85+
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
    "padding": 0
  },
  "subagentStatusLine": {
    "type": "command",
    "command": "~/.claude/claude_subagent_statusline"
  }
}
```

Restart Claude Code for the change to take effect.

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

## Testing manually

```sh
echo '{"workspace":{"current_dir":"/tmp/demo"},"model":{"display_name":"Opus"}}' \
  | ./target/release/claude_statusline

echo '{"columns":80,"tasks":[{"id":"t1","name":"Explore","status":"running","tokenCount":12500,"contextWindowSize":200000}]}' \
  | ./target/release/claude_subagent_statusline
```
