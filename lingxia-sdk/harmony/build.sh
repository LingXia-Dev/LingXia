#!/bin/bash

# Build LingXia SDK HAR for HarmonyOS
# Usage: build.sh

set -euo pipefail

# Parse command line arguments
for arg in "$@"; do
  case "$arg" in
    skip-rust) echo "ℹ️  Rust build is handled by consuming apps; ignoring skip-rust flag." ;;
    *) echo "Usage: $0" >&2; exit 1 ;;
  esac
done

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
LINGXIA_ROOT="$SCRIPT_DIR/../.."
SDK_DIR="$SCRIPT_DIR"

# 2) Build HAR and publish to workspace local repo (target/ohpm)
echo "[2/2] Building HAR ..."
(cd "$SDK_DIR" && hvigorw assembleHar)

HAR_OUT_DIR="$LINGXIA_ROOT/target/ohpm"
mkdir -p "$HAR_OUT_DIR"
HAR_BUNDLE=$(find "$SDK_DIR/lingxia/build" -type f -name "*.har" | head -n1)
if [ -z "${HAR_BUNDLE:-}" ]; then
  echo "❌ HAR not found under $SDK_DIR/lingxia/build" >&2; exit 1
fi
cp "$HAR_BUNDLE" "$HAR_OUT_DIR/lingxia.har"
echo "✅ HAR published to $HAR_OUT_DIR/lingxia.har"
