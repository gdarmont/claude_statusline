#!/usr/bin/env bash
# Install a prebuilt claude_statusline release into ~/.claude.
#
#   curl -fsSL https://raw.githubusercontent.com/gdarmont/claude_statusline/master/install.sh | bash
#
# Environment:
#   CLAUDE_DIR  where to install (default: ~/.claude)
#   VERSION     release tag to install (default: the latest release)
#
# To build from a source checkout instead, use ./dev-install.sh.
set -euo pipefail

REPO="gdarmont/claude_statusline"
DEST="${CLAUDE_DIR:-$HOME/.claude}"

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

if [ "$os" = linux ] && [ "$arch" = aarch64 ]; then
  die "no prebuilt binary for aarch64 Linux. Build from source: https://github.com/$REPO"
fi

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
trap 'rm -rf "$tmp"' EXIT

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

install -m 755 "$tmp/unpacked/claude_statusline" "$DEST/claude_statusline"
install -m 755 "$tmp/unpacked/claude_subagent_statusline" "$DEST/claude_subagent_statusline"

printf '\nInstalled %s -> %s\n' "$version" "$DEST"
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
