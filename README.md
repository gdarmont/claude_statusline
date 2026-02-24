# Claude Statusline

A powerline-style statusline renderer for [Claude Code](https://docs.anthropic.com/en/docs/claude-code). It reads session data as JSON from stdin and outputs ANSI-colored, powerline-styled text.

```
 myproject  main  Claude Opus 4.6  $0.42  37%
```

## Sections

| Section | Color | Description |
|---------|-------|-------------|
| Directory | Blue | Current working directory name |
| Git branch | Green | Current branch (hidden outside git repos) |
| Model | Gray | Active model name |
| Cost | Purple | Session cost in USD (hidden when $0.00) |
| Context | Green / Yellow / Red | Context window usage percentage |

Context window color thresholds:
- **Green** -- 0-59%
- **Yellow** -- 60-80%
- **Red** -- above 80%

## Requirements

- Rust 1.85+
- A terminal with true color (24-bit) support
- A font with powerline glyphs (e.g. Nerd Font)

## Build

```sh
cargo build --release
```

The binary is produced at `target/release/claude_statusline`.

## Install

Copy the binary somewhere on your path or into `~/.claude/`:

```sh
cp target/release/claude_statusline ~/.claude/claude_statusline
```

## Configure Claude Code

Add the following to your Claude Code settings file (`~/.claude/settings.json`):

```json
{
  "statusLine": {
    "type": "command",
    "command": "~/.claude/claude_statusline",
    "padding": 0
  }
}
```

Restart Claude Code for the change to take effect.

## Input format

Claude Code pipes a JSON object to the binary's stdin. The relevant fields are:

```json
{
  "workspace": {
    "current_dir": "/home/user/projects/myproject"
  },
  "model": {
    "display_name": "Claude Opus 4.6"
  },
  "cost": {
    "total_cost_usd": 0.42
  },
  "context_window": {
    "used_percentage": 37.2
  }
}
```

You can test it manually:

```sh
echo '{"workspace":{"current_dir":"/tmp/demo"},"model":{"display_name":"Opus"}}' \
  | ./target/release/claude_statusline
```