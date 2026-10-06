#!/usr/bin/env bash
# Build both binaries in release mode, back up the installed statusline once,
# replace it, smoke-test what actually landed in ~/.claude, then offer to set it
# up in settings.json the way install.sh does.
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
for bin in claude_statusline claude_subagent_statusline; do
  install -m 755 "target/release/$bin" "$DEST/.$bin.new"
  mv -f "$DEST/.$bin.new" "$DEST/$bin"
done
echo "installed -> $DEST"

echo
echo "--- claude_statusline (COLUMNS=100) ---"
COLUMNS=100 "$DEST/claude_statusline" <<'JSON'
{"session_name":"demo","workspace":{"current_dir":"/home/user/dev/myproject"},
 "model":{"display_name":"Opus"},"effort":{"level":"high"},"fast_mode":true,
 "cost":{"total_cost_usd":0.42,"total_duration_ms":754000,"total_lines_added":156,"total_lines_removed":23},
 "context_window":{"total_input_tokens":74500,"context_window_size":200000,"used_percentage":37.2},
 "rate_limits":{"five_hour":{"used_percentage":23.5},"seven_day":{"used_percentage":41.2}}}
JSON

echo
echo "--- claude_subagent_statusline ---"
"$DEST/claude_subagent_statusline" <<'JSON'
{"columns":80,"tasks":[{"id":"t1","name":"Explore","status":"running","label":"scanning src/",
 "model":"claude-opus-5","effort":"high","contextWindowSize":200000,"tokenCount":12500}]}
JSON

CLAUDE_DIR="$DEST" ./install.sh --settings-only
