#!/bin/bash

# Build LingXia SDK HAR for HarmonyOS
#
# Architecture:
#   SDK HAR (lingxia-sdk/harmony/lingxia/):
#     - ArkTS module: LingXia framework
#     - Rust .so: libexample_lxapp.so (user extensions + framework core)
#
# Usage: build.sh [skip-rust]

set -euo pipefail

# Parse command line arguments
SKIP_RUST=false
for arg in "$@"; do
  case "$arg" in
    skip-rust) SKIP_RUST=true; echo "🚀 Skipping Rust compilation" ;;
    *) echo "Usage: $0 [skip-rust]" >&2; exit 1 ;;
  esac
done

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
LINGXIA_ROOT="$SCRIPT_DIR/../.."
SDK_DIR="$SCRIPT_DIR"
LXAPP_FEATURES="${LXAPP_FEATURES:-}" # set via env var, e.g. LXAPP_FEATURES=cloud ./build.sh

# 1) Build Rust (unless skipped) and stage .so into HAR module
if [ "$SKIP_RUST" = false ]; then
  echo "[1/3] Building Rust libraries..."
  if [ -z "${OHOS_NDK_HOME:-}" ]; then
    echo "❌ OHOS_NDK_HOME is not set" >&2; exit 1
  fi

  TARGET="aarch64-unknown-linux-ohos"

  # Toolchain + bindgen headers
  export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_OHOS_LINKER="$OHOS_NDK_HOME/native/llvm/bin/aarch64-unknown-linux-ohos-clang"
  export AR_aarch64_unknown_linux_ohos="$OHOS_NDK_HOME/native/llvm/bin/llvm-ar"
  export CC_aarch64_unknown_linux_ohos="$OHOS_NDK_HOME/native/llvm/bin/aarch64-unknown-linux-ohos-clang"
  export CXX_aarch64_unknown_linux_ohos="$OHOS_NDK_HOME/native/llvm/bin/aarch64-unknown-linux-ohos-clang++"
  SYSROOT="$OHOS_NDK_HOME/native/sysroot"
  export CPATH="$SYSROOT/usr/include:$SYSROOT/usr/include/aarch64-linux-ohos"
  export BINDGEN_EXTRA_CLANG_ARGS="--sysroot=$SYSROOT -I$SYSROOT/usr/include -I$SYSROOT/usr/include/aarch64-linux-ohos"

  # Build lingxia-lib (native library + user extensions)
  cd "$LINGXIA_ROOT"
  if [ -n "$LXAPP_FEATURES" ]; then
    echo "  → Building lingxia-lib with features: $LXAPP_FEATURES"
    cargo build --target $TARGET --release -p lingxia-lib --features "$LXAPP_FEATURES"
  else
    echo "  → Building lingxia-lib..."
    cargo build --target $TARGET --release -p lingxia-lib
  fi

  echo "✅ Rust build complete"
else
  echo "⏭️  Skipping Rust compilation (using existing libraries)"
fi

TARGET="aarch64-unknown-linux-ohos"
SO_SRC="$LINGXIA_ROOT/target/$TARGET/release/liblingxia_lib.so"
SO_DST="$SDK_DIR/lingxia/libs/arm64-v8a/liblingxia.so"
mkdir -p "$(dirname "$SO_DST")" && cp "$SO_SRC" "$SO_DST"
echo "   .so staged: $SO_DST"

# 2) Copy core assets into HAR resources
echo "[2/3] Packing core assets into HAR ..."
SDK_RAW="$SDK_DIR/lingxia/src/main/resources/rawfile"
mkdir -p "$SDK_RAW"
cp "$LINGXIA_ROOT/lingxia-view/404.html" "$SDK_RAW/"
cp "$LINGXIA_ROOT/lingxia-view/webview-bridge.js" "$SDK_RAW/"

# 3) Build HAR and publish to workspace local repo (target/ohpm)
echo "[3/3] Building HAR ..."
(cd "$SDK_DIR" && hvigorw assembleHar)

HAR_OUT_DIR="$LINGXIA_ROOT/target/ohpm"
mkdir -p "$HAR_OUT_DIR"
HAR_BUNDLE=$(find "$SDK_DIR/lingxia/build" -type f -name "*.har" | head -n1)
if [ -z "${HAR_BUNDLE:-}" ]; then
  echo "❌ HAR not found under $SDK_DIR/lingxia/build" >&2; exit 1
fi
cp "$HAR_BUNDLE" "$HAR_OUT_DIR/lingxia.har"
echo "✅ HAR published to $HAR_OUT_DIR/lingxia.har"
