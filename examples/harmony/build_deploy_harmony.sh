#!/bin/bash

# Build and deploy LingXia Example HarmonyOS App
# Usage: ./build_deploy_harmony.sh [skip-rust]

set -euo pipefail

# Parse command line arguments
SKIP_RUST=false
for arg in "$@"; do
  case "$arg" in
    skip-rust)
      SKIP_RUST=true
      echo "🚀 Skipping Rust compilation"
      ;;
    *)
      echo "Unknown argument: $arg"
      echo "Usage: $0 [skip-rust]"
      exit 1
      ;;
  esac
done

# Paths
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
LINGXIA_ROOT="$SCRIPT_DIR/../.."
SDK_DIR="$LINGXIA_ROOT/lingxia-sdk/harmony"

# Native library paths
# Note: SDK HAR does NOT bundle .so; example app directly includes it
RUST_SO_OUTPUT="$LINGXIA_ROOT/target/aarch64-unknown-linux-ohos/release/liblingxia_lib.so"
APP_SO_DEST="$SCRIPT_DIR/entry/libs/arm64-v8a/liblingxia.so"

LXAPP_FEATURES="${LXAPP_FEATURES:-}" # set via env var, e.g. LXAPP_FEATURES=cloud ./build_deploy_harmony.sh

# Example app config
APP_PACKAGE="app.lingxia.lxapp.example"
APP_ABILITY="EntryAbility"
HAP_PATH="$SCRIPT_DIR/entry/build/default/outputs/default/entry-default-signed.hap"

# Helpers
build_rust() {
  if [ -z "${OHOS_NDK_HOME:-}" ]; then
    echo "❌ OHOS_NDK_HOME not set; cannot build Rust" >&2
    exit 1
  fi
  echo "Building Rust libraries (aarch64-unknown-linux-ohos)..."
  TARGET="aarch64-unknown-linux-ohos"
  export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_OHOS_LINKER="$OHOS_NDK_HOME/native/llvm/bin/aarch64-unknown-linux-ohos-clang"
  export AR_aarch64_unknown_linux_ohos="$OHOS_NDK_HOME/native/llvm/bin/llvm-ar"
  export CC_aarch64_unknown_linux_ohos="$OHOS_NDK_HOME/native/llvm/bin/aarch64-unknown-linux-ohos-clang"
  export CXX_aarch64_unknown_linux_ohos="$OHOS_NDK_HOME/native/llvm/bin/aarch64-unknown-linux-ohos-clang++"
  SYSROOT="$OHOS_NDK_HOME/native/sysroot"
  export CPATH="$SYSROOT/usr/include:$SYSROOT/usr/include/aarch64-linux-ohos"
  export BINDGEN_EXTRA_CLANG_ARGS="--sysroot=$SYSROOT -I$SYSROOT/usr/include -I$SYSROOT/usr/include/aarch64-linux-ohos"
  cd "$LINGXIA_ROOT"
  if [ -n "$LXAPP_FEATURES" ]; then
    echo "  → Building lingxia-lib with features: $LXAPP_FEATURES"
    cargo build --target $TARGET --release -p lingxia-lib --features "$LXAPP_FEATURES"
  else
    echo "  → Building lingxia-lib..."
    cargo build --target $TARGET --release -p lingxia-lib
  fi
  echo "✅ Rust build complete"
}

stage_so() {
  if [ ! -f "$RUST_SO_OUTPUT" ]; then
    echo "❌ Native library not found: $RUST_SO_OUTPUT" >&2
    exit 1
  fi
  mkdir -p "$(dirname "$APP_SO_DEST")"
  cp "$RUST_SO_OUTPUT" "$APP_SO_DEST"
  echo "   ✅ Native library staged: $APP_SO_DEST"
}

# Clean previous HAR/build outputs to ensure a fresh bundle
HAR_BUNDLE="$LINGXIA_ROOT/target/ohpm/lingxia.har"
SDK_BUILD_SH="$LINGXIA_ROOT/lingxia-sdk/harmony/build.sh"
echo "Cleaning previous HAR artifacts..."
rm -f "$HAR_BUNDLE" 2>/dev/null || true
rm -rf "$LINGXIA_ROOT/lingxia-sdk/harmony/lingxia/build" 2>/dev/null || true

# 1) Build Rust native library
if [ "$SKIP_RUST" = false ]; then
  build_rust
else
  echo "⏭️  Skipping Rust compilation (using existing .so from target/)"
fi
stage_so

# 2) Build SDK HAR (ArkTS only, no native library bundled)
echo "Building SDK HAR (ArkTS framework only)..."
bash "$SDK_BUILD_SH" skip-rust || { echo "❌ Failed to build SDK HAR" >&2; exit 1; }

if [ ! -f "$HAR_BUNDLE" ]; then
  echo "❌ HAR not found after build: $HAR_BUNDLE" >&2; exit 1
fi

# 3) Prepare example app assets (app.json + homelxapp)
echo "Preparing example app assets (app.json + homelxapp) ..."
RAWFILE_DIR="$SCRIPT_DIR/entry/src/main/resources/rawfile"
mkdir -p "$RAWFILE_DIR" && rm -rf "$RAWFILE_DIR"/*
cp "$LINGXIA_ROOT/examples/demo/app.json" "$RAWFILE_DIR/"
if [ -d "$LINGXIA_ROOT/examples/demo/homelxapp/dist" ]; then
  mkdir -p "$RAWFILE_DIR/homelxapp" && cp -R "$LINGXIA_ROOT/examples/demo/homelxapp/dist/"* "$RAWFILE_DIR/homelxapp/"
fi

# 4) Build & install example HAP
echo "Installing ohpm dependencies (local har) ..."
(cd "$SCRIPT_DIR/entry" && ohpm install)

echo "Building example HAP ..."
(cd "$SCRIPT_DIR" && hvigorw assembleHap)

echo "Installing HAP ..."
if ! command -v hdc >/dev/null 2>&1; then
  echo "❌ hdc not found in PATH" >&2; exit 1
fi
if ! hdc list targets | grep -q ".*"; then
  echo "❌ No HarmonyOS device connected (hdc list targets empty)" >&2; exit 1
fi
hdc install -r "$HAP_PATH" >/dev/null

echo "Starting app ..."
hdc shell aa start -a "$APP_ABILITY" -b "$APP_PACKAGE" >/dev/null

echo "Showing logs (Ctrl-C to stop; auto-stop after ${HILOG_DURATION:-60}s)..."

DURATION="${HILOG_DURATION:-60}"
TAGS_CSV="${HILOG_TAGS:-LingXia,LxApp,WebView}"
IFS=',' read -r -a TAGS_ARR <<< "$TAGS_CSV"
PATTERN="$(printf '%s|' "${TAGS_ARR[@]}")"; PATTERN="${PATTERN%|}"

# Stream via FIFO so we hold hdc PID and can terminate it on Ctrl-C
TMP_DIR="$(mktemp -d 2>/dev/null || mktemp -d -t lingxia_hilog)"
PIPE="$TMP_DIR/hilog.pipe"
mkfifo "$PIPE" || { echo "❌ Failed to create log pipe" >&2; exit 1; }

cleanup_logs() {
  echo; echo "Stopping logs..."
  [ -n "${HILOG_PID:-}" ] && kill "${HILOG_PID}" 2>/dev/null || true
  rm -f "$PIPE" 2>/dev/null || true
  rmdir "$TMP_DIR" 2>/dev/null || true
}
trap cleanup_logs INT TERM

hdc hilog > "$PIPE" &
HILOG_PID=$!

if [ "$DURATION" -gt 0 ] 2>/dev/null; then
  ( sleep "$DURATION"; cleanup_logs ) &
fi

grep -E "(${PATTERN})" < "$PIPE" || true
cleanup_logs

echo "✅ Done."
