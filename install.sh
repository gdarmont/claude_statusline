#!/usr/bin/env bash
# Install a prebuilt claude_statusline release into ~/.claude.
#
#   curl -fsSL https://raw.githubusercontent.com/gdarmont/claude_statusline/master/install.sh | bash
#
# Environment:
#   CLAUDE_DIR      where to install (default: ~/.claude)
#   VERSION         release tag to install (default: the latest release)
#   NONINTERACTIVE  1 to skip the questions about optional settings
#
# To build from a source checkout instead, use ./dev-install.sh.
set -euo pipefail

REPO="gdarmont/claude_statusline"
DEST="${CLAUDE_DIR:-$HOME/.claude}"
settings="${CLAUDE_CONFIG_DIR:-$HOME/.claude}/settings.json"

die() { printf 'error: %s\n' "$1" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || die "missing required command: $1"; }

need curl
need unzip

case "$(uname -s)" in
  Linux)  os=linux ;;
  Darwin) os=macos ;;
  *)      die "unsupported OS: $(uname -s). Build from source: https://github.com/$REPO" ;;
esac

case "$(uname -m)" in
  x86_64 | amd64)  arch=x86_64 ;;
  arm64 | aarch64) arch=aarch64 ;;
  *)               die "unsupported architecture: $(uname -m)" ;;
esac

version="${VERSION:-}"
if [ -z "$version" ]; then
  # Follow the /releases/latest redirect rather than hitting the API, which
  # rate-limits unauthenticated callers to 60 requests an hour.
  version=$(curl -fsSLI -o /dev/null -w '%{url_effective}' \
    "https://github.com/$REPO/releases/latest" | sed 's|.*/tag/||')
  [ -n "$version" ] || die "could not determine the latest release"
fi

asset="claude_statusline-$version-$arch-$os.zip"
base="https://github.com/$REPO/releases/download/$version"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp" "$DEST/.claude_statusline.new" "$DEST/.claude_subagent_statusline.new" "$settings.claude_statusline.new"' EXIT

printf 'Downloading %s (%s)...\n' "$asset" "$version"
curl -fsSL -o "$tmp/$asset" "$base/$asset" \
  || die "no asset $asset in release $version"

if curl -fsSL -o "$tmp/SHA256SUMS" "$base/SHA256SUMS"; then
  if command -v sha256sum >/dev/null 2>&1; then
    actual=$(sha256sum "$tmp/$asset" | cut -d' ' -f1)
  else
    actual=$(shasum -a 256 "$tmp/$asset" | cut -d' ' -f1)
  fi
  expected=$(grep " $asset\$" "$tmp/SHA256SUMS" | cut -d' ' -f1)
  [ -n "$expected" ] || die "$asset is missing from SHA256SUMS"
  [ "$actual" = "$expected" ] || die "checksum mismatch for $asset"
  printf 'Checksum verified.\n'
else
  printf 'warning: release %s publishes no SHA256SUMS, skipping verification\n' "$version" >&2
fi

unzip -q -o "$tmp/$asset" -d "$tmp/unpacked"

mkdir -p "$DEST"
# Keep a single pristine backup of whatever was installed before the first run.
if [ -f "$DEST/claude_statusline" ] && [ ! -f "$DEST/claude_statusline.bak" ]; then
  cp -p "$DEST/claude_statusline" "$DEST/claude_statusline.bak"
  printf 'Backed up previous binary -> %s/claude_statusline.bak\n' "$DEST"
fi

# Write next to the target, then rename over it: a status line render that
# starts mid-update runs the old binary or the new one, never a partial file.
for bin in claude_statusline claude_subagent_statusline; do
  install -m 755 "$tmp/unpacked/$bin" "$DEST/.$bin.new"
  mv -f "$DEST/.$bin.new" "$DEST/$bin"
done

printf '\nInstalled %s -> %s\n' "$version" "$DEST"

# Optional settings, read from the "env" block of settings.json. Each is offered
# once: an answer, even a no, is saved, so updates don't ask again.
defined() {
  [ -n "${!1+set}" ] && return 0
  if command -v jq >/dev/null 2>&1; then
    jq -e --arg key "$1" '.env | has($key)' "$settings" >/dev/null 2>&1
  else
    grep -qF "\"$1\"" "$settings" 2>/dev/null
  fi
}

# Under `curl | bash`, stdin is the script itself, so questions go through the terminal
ask() {
  local answer
  printf '%s' "$1" >/dev/tty
  IFS= read -r answer </dev/tty || answer=""
  printf '%s' "${answer:-$2}"
}

# Merge a JSON object into the "env" block, keeping a backup. A symlinked
# settings.json, from a dotfiles manager, is written through rather than replaced.
save_env() {
  local new="$settings.claude_statusline.new"
  command -v jq >/dev/null 2>&1 || return 1
  mkdir -p "$(dirname "$settings")"
  if [ -f "$settings" ]; then
    # Copied first so the new file keeps its permissions
    cp -p "$settings" "$new"
    jq --argjson add "$1" '.env = ((.env // {}) + $add)' "$settings" >"$new" 2>/dev/null \
      || return 1
    [ -s "$new" ] || return 1
    cp -p "$settings" "$settings.claude_statusline.bak"
    backup=" (previous version: $settings.claude_statusline.bak)"
  else
    jq -n --argjson add "$1" '{env: $add}' >"$new"
  fi
  if [ -L "$settings" ]; then
    cat "$new" >"$settings"
  else
    mv -f "$new" "$settings"
  fi
}

answers=()
unset_settings=()
backup=""
interactive=false
if [ "${NONINTERACTIVE:-}" != 1 ] && (: </dev/tty) 2>/dev/null; then
  interactive=true
fi

if ! defined CLAUDE_STATUSLINE_CACHE_NOTIFY; then
  if $interactive; then
    cat >/dev/tty <<'EOF'

The status line can raise a desktop notification before the prompt cache
expires, so you can reply while it's still warm. The warning must be shorter
than the cache's lifetime: under 5 minutes for a 5-minute cache, under 60 for
a 1-hour one.
EOF
    while :; do
      lead=$(ask 'Minutes of warning, or 0 for no notification [0]: ' 0)
      case "$lead" in
        [0-9] | [0-5][0-9]) lead=$((10#$lead)); break ;;
        *) printf 'Enter a whole number of minutes below 60.\n' >/dev/tty ;;
      esac
    done
    answers+=("CLAUDE_STATUSLINE_CACHE_NOTIFY=$lead")
    if [ "$lead" -gt 0 ] && [ "$os" = linux ] && ! command -v notify-send >/dev/null 2>&1; then
      printf 'warning: notify-send not found; install libnotify for the notification to show\n' >&2
    fi
  else
    unset_settings+=("CLAUDE_STATUSLINE_CACHE_NOTIFY  minutes of warning before the prompt cache expires (default: off)")
  fi
fi

if ! defined CLAUDE_STATUSLINE_UPDATE_CHECK; then
  if $interactive; then
    while :; do
      check=$(ask $'\nCheck GitHub once a day for a newer release of the status line? [Y/n]: ' y)
      case "$check" in
        [Yy] | [Yy][Ee][Ss]) check=1; break ;;
        [Nn] | [Nn][Oo]) check=0; break ;;
      esac
    done
    answers+=("CLAUDE_STATUSLINE_UPDATE_CHECK=$check")
  else
    unset_settings+=("CLAUDE_STATUSLINE_UPDATE_CHECK  0 turns off the daily check for a newer release (default: on)")
  fi
fi

if [ ${#answers[@]} -gt 0 ]; then
  # Keys are fixed and values are validated digits, so nothing needs escaping
  add="{"
  for pair in "${answers[@]}"; do
    add+=$(printf '"%s": "%s", ' "${pair%%=*}" "${pair#*=}")
  done
  add="${add%, }}"
  if save_env "$add"; then
    printf '\nSaved to %s%s\n' "$settings" "$backup"
  else
    printf '\nAdd this to %s, merged into its "env" block if it has one:\n\n  "env": {\n' "$settings"
    last=$((${#answers[@]} - 1))
    for i in "${!answers[@]}"; do
      pair="${answers[$i]}"
      separator=","
      [ "$i" -eq "$last" ] && separator=""
      printf '    "%s": "%s"%s\n' "${pair%%=*}" "${pair#*=}" "$separator"
    done
    printf '  }\n'
  fi
elif [ ${#unset_settings[@]} -gt 0 ]; then
  printf '\nOptional settings, for the "env" block of %s:\n' "$settings"
  printf '  %s\n' "${unset_settings[@]}"
fi

# On an update, settings.json already points here, in full or as ~/...
case "$DEST" in
  "$HOME"/*) short="~${DEST#"$HOME"}" ;;
  *)         short="$DEST" ;;
esac
configured() {
  grep -qF -e "$DEST/$1\"" -e "$short/$1\"" "$settings" 2>/dev/null
}

if configured claude_statusline && configured claude_subagent_statusline; then
  printf 'Claude Code is already set up for it: the next status line refresh runs %s.\n' "$version"
  exit 0
fi

cat <<EOF

Add this to ~/.claude/settings.json, then restart Claude Code:

  "statusLine": {
    "type": "command",
    "command": "$DEST/claude_statusline",
    "padding": 0,
    "refreshInterval": 15
  },
  "subagentStatusLine": {
    "type": "command",
    "command": "$DEST/claude_subagent_statusline"
  }

refreshInterval redraws the status line every 15 seconds, so the prompt cache
countdown keeps running while the session is idle. Already configured? Add it
to your existing statusLine block.
EOF
