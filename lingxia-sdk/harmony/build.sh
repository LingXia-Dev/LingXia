#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Build LingXia Harmony SDK HAR.

Usage:
  build.sh [--skip-rust]

Notes:
  This script only builds the Harmony HAR. Rust .so build is handled by app-side
  LingXia CLI builds, so --skip-rust is accepted only
  for compatibility and has no effect here.
EOF
}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LINGXIA_ROOT="$SCRIPT_DIR/../.."

for arg in "$@"; do
  case "$arg" in
    --skip-rust)
      echo "ℹ️  --skip-rust has no effect in lingxia-sdk/harmony/build.sh (no Rust compile stage)."
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      usage >&2
      exit 1
      ;;
  esac
done

echo "Building Harmony HAR via scripts/release/sdk.sh ..."
bash "$LINGXIA_ROOT/scripts/release/sdk.sh" \
  --platform harmony \
  --no-shasums \
  --out "$LINGXIA_ROOT/target/sdk-harmony-build"

HAR_OUT="$LINGXIA_ROOT/target/ohpm/lingxia.har"
if [[ ! -f "$HAR_OUT" ]]; then
  echo "❌ HAR not found: $HAR_OUT" >&2
  exit 1
fi
echo "✅ HAR published to $HAR_OUT"
