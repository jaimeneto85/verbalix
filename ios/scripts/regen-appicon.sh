#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
IOS_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

COMMON_GIT="$(git -C "$IOS_DIR" rev-parse --git-common-dir 2>/dev/null || true)"
if [[ "$COMMON_GIT" = /* ]]; then
  REPO_ROOT="$(dirname "$COMMON_GIT")"
elif GIT_TOPLEVEL="$(git -C "$IOS_DIR" rev-parse --show-toplevel 2>/dev/null)"; then
  REPO_ROOT="$GIT_TOPLEVEL"
else
  REPO_ROOT="$(cd "$IOS_DIR/../.." && pwd)"
fi

MASTER="$REPO_ROOT/branding/verbalix-app-icon-master.png"
APPICONSET="$IOS_DIR/Verbalix/Assets.xcassets/AppIcon.appiconset/AppIcon-1024.png"

if [[ ! -f "$MASTER" ]]; then
  echo "ERROR: master icon not found at $MASTER" >&2
  exit 1
fi

magick "$MASTER" -background white -alpha remove -alpha off "$APPICONSET"
echo "Regenerated $APPICONSET"
sips -g hasAlpha "$APPICONSET"
sips -g pixelWidth -g pixelHeight "$APPICONSET"
