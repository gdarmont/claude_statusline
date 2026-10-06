#!/usr/bin/env bash
# Install a prebuilt claude_statusline release into ~/.claude, then offer to set
# it up in Claude Code's settings.json.
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

# The rest sets up settings.json: the two status line commands, and the optional
# settings the status line reads from its "env" block. With a terminal and jq, it
# asks, then makes every change in one write; otherwise it prints what to add.

# Under `curl | bash`, stdin is the script itself, so questions go through the terminal
interactive=false
if [ "${NONINTERACTIVE:-}" != 1 ] && (: </dev/tty) 2>/dev/null; then
  interactive=true
fi

ask() {
  local answer
  printf '%s' "$1" >/dev/tty
  IFS= read -r answer </dev/tty || answer=""
  printf '%s' "${answer:-$2}"
}

# jq can edit settings.json when it holds a JSON object, or when it's missing
editable=false
if command -v jq >/dev/null 2>&1; then
  if [ ! -e "$settings" ] || jq -e 'type == "object"' "$settings" >/dev/null 2>&1; then
    editable=true
  else
    printf 'warning: %s is not a JSON object, so it is left as it is\n' "$settings" >&2
  fi
fi

defined() {
  [ -n "${!1+set}" ] && return 0
  if command -v jq >/dev/null 2>&1; then
    jq -e --arg key "$1" '.env | has($key)' "$settings" >/dev/null 2>&1
  else
    grep -qF "\"$1\"" "$settings" 2>/dev/null
  fi
}

# Apply a jq filter to settings.json, keeping a backup. A symlinked settings.json,
# from a dotfiles manager, is written through rather than replaced.
save_settings() {
  local new="$settings.claude_statusline.new"
  mkdir -p "$(dirname "$settings")"
  if [ -f "$settings" ]; then
    # Copied first so the new file keeps its permissions
    cp -p "$settings" "$new"
    jq "$@" "$settings" >"$new" 2>/dev/null || return 1
    [ -s "$new" ] || return 1
    cp -p "$settings" "$settings.claude_statusline.bak"
    backup=" (previous version: $settings.claude_statusline.bak)"
  else
    printf '{}' | jq "$@" >"$new" || return 1
  fi
  if [ -L "$settings" ]; then
    cat "$new" >"$settings"
  else
    mv -f "$new" "$settings"
  fi
}

# Optional settings. Each is offered once: an answer, even a no, is saved, so
# updates don't ask again.
answers=()
unset_settings=()

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

# The status line commands. On an update, settings.json already runs them, in
# full or as ~/...
case "$DEST" in
  "$HOME"/*) short="~${DEST#"$HOME"}" ;;
  *)         short="$DEST" ;;
esac
configured() {
  grep -qF -e "$DEST/$1\"" -e "$short/$1\"" "$settings" 2>/dev/null
}
ours() { [ "$1" = "$DEST/$2" ] || [ "$1" = "$short/$2" ]; }

# The command settings.json runs for a status line, empty when there is none
current_command() {
  [ -f "$settings" ] || return 0
  jq -r --arg key "$1" \
    '.[$key] // empty | if type == "object" then .command // "" else . end | tostring' \
    "$settings"
}

# One line of the plan: add the block, or replace the command it runs now
change() {
  if [ -z "$2" ]; then
    printf '%-18s  add it, running %s' "$1" "$3"
  else
    printf '%-18s  replace %s with %s' "$1" "$2" "$3"
  fi
}

status_ok=false
status_changes=()
replacing=false
status_block=null
sub_block=null
refresh=false
if $editable; then
  status_now=$(current_command statusLine)
  sub_now=$(current_command subagentStatusLine)
  if ! ours "$status_now" claude_statusline; then
    status_block=$(jq -n --arg command "$DEST/claude_statusline" \
      '{type: "command", command: $command, padding: 0, refreshInterval: 15}')
    status_changes+=("$(change statusLine "$status_now" "$DEST/claude_statusline")")
    if [ -n "$status_now" ]; then
      replacing=true
    fi
  elif [ "$(jq '.statusLine | has("refreshInterval")' "$settings")" = false ]; then
    refresh=true
    status_changes+=("$(printf '%-18s  add "refreshInterval": 15, so countdowns run while you are idle' statusLine)")
  fi
  if ! ours "$sub_now" claude_subagent_statusline; then
    sub_block=$(jq -n --arg command "$DEST/claude_subagent_statusline" \
      '{type: "command", command: $command}')
    status_changes+=("$(change subagentStatusLine "$sub_now" "$DEST/claude_subagent_statusline")")
    if [ -n "$sub_now" ]; then
      replacing=true
    fi
  fi
  if [ ${#status_changes[@]} -eq 0 ]; then
    status_ok=true
  fi
elif configured claude_statusline && configured claude_subagent_statusline; then
  status_ok=true
fi

# Replacing another status line is opt-in; adding one is the point of installing
apply_status=false
if [ ${#status_changes[@]} -gt 0 ] && $interactive; then
  printf '\nThe status line needs these changes to settings.json:\n' >/dev/tty
  printf '  %s\n' "${status_changes[@]}" >/dev/tty
  if $replacing; then
    prompt='Apply them? [y/N]: '
    default=n
  else
    prompt='Apply them? [Y/n]: '
    default=y
  fi
  while :; do
    case "$(ask "$prompt" "$default")" in
      [Yy] | [Yy][Ee][Ss]) apply_status=true; break ;;
      [Nn] | [Nn][Oo]) break ;;
    esac
  done
fi
if ! $apply_status; then
  status_block=null
  sub_block=null
  refresh=false
fi

# Keys are fixed and values are validated digits, so nothing needs escaping
env_add="{"
for pair in ${answers[@]+"${answers[@]}"}; do
  env_add+=$(printf '"%s": "%s", ' "${pair%%=*}" "${pair#*=}")
done
env_add="${env_add%, }}"

backup=""
saved=false
if $editable && { [ ${#answers[@]} -gt 0 ] || $apply_status; }; then
  # shellcheck disable=SC2016 # jq variables, not shell ones
  if save_settings --argjson env "$env_add" --argjson status "$status_block" \
    --argjson sub "$sub_block" --argjson refresh "$refresh" '
      (if $env == {} then . else .env = ((.env // {}) + $env) end)
      | (if $status == null then . else .statusLine = $status end)
      | (if $sub == null then . else .subagentStatusLine = $sub end)
      | (if $refresh then .statusLine.refreshInterval = 15 else . end)'; then
    saved=true
    printf '\nSaved to %s%s\n' "$settings" "$backup"
  fi
fi

if ! $saved && [ ${#answers[@]} -gt 0 ]; then
  printf '\nAdd this to %s, merged into its "env" block if it has one:\n\n  "env": {\n' "$settings"
  last=$((${#answers[@]} - 1))
  for i in "${!answers[@]}"; do
    pair="${answers[$i]}"
    separator=","
    [ "$i" -eq "$last" ] && separator=""
    printf '    "%s": "%s"%s\n' "${pair%%=*}" "${pair#*=}" "$separator"
  done
  printf '  }\n'
elif [ ${#unset_settings[@]} -gt 0 ]; then
  printf '\nOptional settings, for the "env" block of %s:\n' "$settings"
  printf '  %s\n' "${unset_settings[@]}"
fi

if $status_ok; then
  printf '\nClaude Code is already set up for it: the next status line refresh runs %s.\n' "$version"
  exit 0
fi
if $saved && $apply_status; then
  printf 'Restart Claude Code for the status line changes to take effect.\n'
  exit 0
fi
if [ ${#status_changes[@]} -gt 0 ] && $interactive; then
  printf '\nLeft the status line settings as they were.\n'
  exit 0
fi

cat <<EOF

Add this to $settings, then restart Claude Code:

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
