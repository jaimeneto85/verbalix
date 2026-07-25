#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
IOS_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
ROOT_ENV="$(cd "$IOS_DIR/../.." && pwd)/.env"

if [[ ! -f "$ROOT_ENV" ]]; then
  echo "ERROR: .env not found at $ROOT_ENV" >&2
  echo "Copy .env.example to .env and fill in VITE_SUPABASE_URL and VITE_SUPABASE_ANON_KEY." >&2
  exit 1
fi

SUPABASE_URL=""
SUPABASE_ANON_KEY=""

while IFS='=' read -r key value; do
  [[ "$key" =~ ^[[:space:]]*# ]] && continue
  [[ -z "$key" ]] && continue
  key="${key// /}"
  value="${value// /}"
  case "$key" in
    VITE_SUPABASE_URL) SUPABASE_URL="$value" ;;
    VITE_SUPABASE_ANON_KEY) SUPABASE_ANON_KEY="$value" ;;
  esac
done < "$ROOT_ENV"

if [[ -z "$SUPABASE_URL" ]]; then
  echo "ERROR: VITE_SUPABASE_URL is missing or empty in $ROOT_ENV" >&2
  exit 1
fi

if [[ -z "$SUPABASE_ANON_KEY" ]]; then
  echo "ERROR: VITE_SUPABASE_ANON_KEY is missing or empty in $ROOT_ENV" >&2
  exit 1
fi

CONFIG_DIR="$IOS_DIR/Config"
mkdir -p "$CONFIG_DIR"

cat > "$CONFIG_DIR/Supabase.xcconfig" <<XCCONFIG
VerbalixSupabaseURL = $SUPABASE_URL
VerbalixSupabaseAnonKey = $SUPABASE_ANON_KEY
XCCONFIG

echo "Generated ios/Config/Supabase.xcconfig"

xcodegen generate --spec "$IOS_DIR/project.yml"
echo "Xcode project regenerated."
