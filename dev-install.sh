#!/usr/bin/env bash
# Build the binary in release mode, back up the installed one once, replace it,
# smoke-test what actually landed in ~/.claude, then offer to set it up in
# settings.json the way install.sh does.
set -euo pipefail

cd "$(dirname "$0")"
DEST="${CLAUDE_DIR:-$HOME/.claude}"

cargo build --release

# Keep a single pristine backup of whatever was installed before the first run.
if [ -f "$DEST/claude_statusline" ] && [ ! -f "$DEST/claude_statusline.bak" ]; then
  cp -p "$DEST/claude_statusline" "$DEST/claude_statusline.bak"
  echo "backed up previous binary -> $DEST/claude_statusline.bak"
fi

# Write next to the target, then rename over it: a status line render that
# starts mid-update runs the old binary or the new one, never a partial file.
install -m 755 target/release/claude_statusline "$DEST/.claude_statusline.new"
mv -f "$DEST/.claude_statusline.new" "$DEST/claude_statusline"
echo "installed -> $DEST"

# Releases up to v1.5.0 also installed claude_subagent_statusline, for the
# subagentStatusLine setting that Claude Code v2.1.323 removed
for old in "$DEST/claude_subagent_statusline" "$DEST/claude_subagent_statusline.bak"; do
  if [ -e "$old" ]; then
    rm -f "$old"
    echo "removed $old"
  fi
done

echo
echo "--- claude_statusline (COLUMNS=100) ---"
COLUMNS=100 "$DEST/claude_statusline" <<'JSON'
{"session_name":"demo","workspace":{"current_dir":"/home/user/dev/myproject"},
 "model":{"display_name":"Opus"},"effort":{"level":"high"},"fast_mode":true,
 "cost":{"total_cost_usd":0.42,"total_duration_ms":754000,"total_lines_added":156,"total_lines_removed":23},
 "context_window":{"total_input_tokens":74500,"context_window_size":200000,"used_percentage":37.2},
 "rate_limits":{"five_hour":{"used_percentage":23.5},"seven_day":{"used_percentage":41.2}}}
JSON

CLAUDE_DIR="$DEST" ./install.sh --settings-only
