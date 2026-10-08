#!/usr/bin/env bash
# Remove claude_statusline: the two binaries, the files the status line keeps in
# Claude Code's config directory, and its settings in settings.json.
#
#   curl -fsSL https://raw.githubusercontent.com/gdarmont/claude_statusline/master/uninstall.sh | bash
#
# Environment:
#   CLAUDE_DIR      where it was installed (default: ~/.claude)
#   NONINTERACTIVE  1 to remove everything without asking, as happens with no terminal
set -euo pipefail

DEST="${CLAUDE_DIR:-$HOME/.claude}"
config="${CLAUDE_CONFIG_DIR:-$HOME/.claude}"
settings="$config/settings.json"

die() { printf 'error: %s\n' "$1" >&2; exit 1; }

[ $# -eq 0 ] || die "unknown option: $1 (uninstall.sh takes none)"

trap 'rm -f "$settings.claude_statusline.new"' EXIT

# The binaries, their .bak backups, and what the status line
# writes as it runs: the update check's cache and the cache notification markers
files=()
for path in "$DEST/claude_statusline" "$DEST/claude_subagent_statusline" \
  "$DEST/claude_statusline.bak" "$DEST/claude_subagent_statusline.bak" \
  "$config/claude_statusline.update" "$config/claude_statusline.update.partial" \
  "$config/claude_statusline.cache-notify"; do
  if [ -e "$path" ] || [ -L "$path" ]; then
    files+=("$path")
  fi
done

# Under `curl | bash`, stdin is the script itself, so the question goes through the terminal
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

# A status line runs ours when its command ends in the binary's name: the
# installed copy, in full or as ~/..., or a source build
ours() {
  case "$1" in
    "$2" | */"$2") return 0 ;;
    *) return 1 ;;
  esac
}

# The command settings.json runs for a status line, empty when there is none
current_command() {
  jq -r --arg key "$1" \
    '.[$key] // empty | if type == "object" then .command // "" else . end | tostring' \
    "$settings"
}

# Apply a jq filter to settings.json, keeping a backup. A symlinked settings.json,
# from a dotfiles manager, is written through rather than replaced.
save_settings() {
  local new="$settings.claude_statusline.new"
  # Copied first so the new file keeps its permissions
  cp -p "$settings" "$new"
  jq "$@" "$settings" >"$new" 2>/dev/null || return 1
  [ -s "$new" ] || return 1
  cp -p "$settings" "$settings.claude_statusline.bak"
  if [ -L "$settings" ]; then
    cat "$new" >"$settings"
  else
    mv -f "$new" "$settings"
  fi
}

# What to take out of settings.json. With jq and a JSON object, it is edited;
# otherwise, when it mentions the status line, the steps are printed instead.
changes=()
drop_status=false
drop_sub=false
drop_env=false
manual=""
if [ -f "$settings" ]; then
  if ! command -v jq >/dev/null 2>&1; then
    manual="jq is not installed"
  elif ! jq -e 'type == "object"' "$settings" >/dev/null 2>&1; then
    manual="it is not a JSON object"
  else
    status_now=$(current_command statusLine)
    if ours "$status_now" claude_statusline; then
      drop_status=true
      changes+=("$(printf '%-18s  remove it, which runs %s' statusLine "$status_now")")
    fi
    sub_now=$(current_command subagentStatusLine)
    if ours "$sub_now" claude_subagent_statusline; then
      drop_sub=true
      changes+=("$(printf '%-18s  remove it, which runs %s' subagentStatusLine "$sub_now")")
    fi
    env_now=$(jq -r '.env | if type == "object"
      then [keys[] | select(startswith("CLAUDE_STATUSLINE_"))] | join(", ")
      else "" end' "$settings")
    if [ -n "$env_now" ]; then
      drop_env=true
      changes+=("$(printf '%-18s  remove %s' env "$env_now")")
    fi
  fi
  if [ -n "$manual" ] && ! grep -qF -e claude_statusline -e claude_subagent_statusline \
    -e CLAUDE_STATUSLINE_ "$settings"; then
    manual=""
  fi
fi

if [ ${#files[@]} -eq 0 ] && [ ${#changes[@]} -eq 0 ] && [ -z "$manual" ]; then
  printf 'Nothing to remove: %s holds no claude_statusline, and %s does not mention it.\n' \
    "$DEST" "$settings"
  exit 0
fi

if [ ${#files[@]} -gt 0 ]; then
  printf 'Files to remove:\n'
  printf '  %s\n' "${files[@]}"
  echo
fi
if [ ${#changes[@]} -gt 0 ]; then
  printf 'Changes to %s:\n' "$settings"
  printf '  %s\n' "${changes[@]}"
  echo
fi
if [ -n "$manual" ]; then
  printf '%s mentions the status line, but %s, so it is left for you to edit.\n\n' \
    "$settings" "$manual"
fi

if $interactive; then
  while :; do
    case "$(ask 'Uninstall? [Y/n]: ' y)" in
      [Yy] | [Yy][Ee][Ss]) break ;;
      [Nn] | [Nn][Oo]) printf 'Left everything as it was.\n'; exit 0 ;;
    esac
  done
fi

# settings.json first: if it can't be written, the binaries it runs stay in place
if [ ${#changes[@]} -gt 0 ]; then
  # shellcheck disable=SC2016 # jq variables, not shell ones
  save_settings --argjson status "$drop_status" --argjson sub "$drop_sub" \
    --argjson env "$drop_env" '
      (if $status then del(.statusLine) else . end)
      | (if $sub then del(.subagentStatusLine) else . end)
      | (if $env then
          .env |= with_entries(select(.key | startswith("CLAUDE_STATUSLINE_") | not))
          | if .env == {} then del(.env) else . end
        else . end)' \
    || die "could not write $settings, so nothing was removed"
  printf 'Saved %s (previous version: %s.claude_statusline.bak)\n' "$settings" "$settings"
fi

if [ ${#files[@]} -gt 0 ]; then
  rm -rf -- "${files[@]}"
  printf 'Removed claude_statusline.\n'
fi

if [ -n "$manual" ]; then
  cat <<EOF

Remove these from $settings, then restart Claude Code:

  - the "statusLine" and "subagentStatusLine" blocks, if they run claude_statusline
  - the CLAUDE_STATUSLINE_* entries in the "env" block
EOF
elif [ ${#changes[@]} -gt 0 ]; then
  printf 'Restart Claude Code for the status line changes to take effect.\n'
fi
