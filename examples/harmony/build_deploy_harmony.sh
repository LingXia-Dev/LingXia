#!/bin/bash

set -euo pipefail

# Usage: build_deploy_harmony.sh [skip-rust]
SKIP_RUST=false
for arg in "$@"; do
  case "$arg" in
    skip-rust) SKIP_RUST=true; echo "⏭️  Skipping Rust compilation" ;;
    *) echo "Usage: $0 [skip-rust]" >&2; exit 1 ;;
  esac
done

# Paths
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
LINGXIA_ROOT="$SCRIPT_DIR/../.."
SDK_DIR="$LINGXIA_ROOT/lingxia-sdk/harmony"

# Example app config
APP_PACKAGE="app.lingxia.lxapp.example"
APP_ABILITY="EntryAbility"
HAP_PATH="$SCRIPT_DIR/entry/build/default/outputs/default/entry-default-signed.hap"

# 1) Build/ensure SDK HAR
HAR_BUNDLE="$LINGXIA_ROOT/target/ohpm/lingxia.har"
SDK_BUILD_SH="$LINGXIA_ROOT/lingxia-sdk/harmony/build.sh"

if [ "$SKIP_RUST" = false ]; then
  echo "[0/3] Building SDK HAR (full) ..."
  if [ -z "${OHOS_NDK_HOME:-}" ]; then
    echo "❌ OHOS_NDK_HOME not set; cannot build Rust for HAR" >&2; exit 1
  fi
  bash "$SDK_BUILD_SH" || { echo "❌ Failed to build SDK HAR" >&2; exit 1; }
else
  if [ ! -f "$HAR_BUNDLE" ]; then
    echo "[0/3] SDK HAR not found. Building SDK HAR (skip-rust) ..."
    bash "$SDK_BUILD_SH" skip-rust || { echo "❌ Failed to build SDK HAR (skip-rust)" >&2; exit 1; }
  fi
fi

if [ ! -f "$HAR_BUNDLE" ]; then
  echo "❌ HAR not found after build: $HAR_BUNDLE" >&2; exit 1
fi

# Prevent duplicate native libs in example (SDK HAR already includes liblingxia.so)
if [ -f "$SCRIPT_DIR/entry/libs/arm64-v8a/liblingxia.so" ]; then
  rm -f "$SCRIPT_DIR/entry/libs/arm64-v8a/liblingxia.so"
fi

# 2) Prepare example app assets (app.json + homelxapp)
echo "[1/3] Preparing example app assets (app.json + homelxapp) ..."
RAWFILE_DIR="$SCRIPT_DIR/entry/src/main/resources/rawfile"
mkdir -p "$RAWFILE_DIR" && rm -rf "$RAWFILE_DIR"/*
cp "$LINGXIA_ROOT/examples/demo/app.json" "$RAWFILE_DIR/"
if [ -d "$LINGXIA_ROOT/examples/demo/homelxapp/dist" ]; then
  mkdir -p "$RAWFILE_DIR/homelxapp" && cp -R "$LINGXIA_ROOT/examples/demo/homelxapp/dist/"* "$RAWFILE_DIR/homelxapp/"
fi

# 4) Build & install example HAP
echo "[2/3] Installing ohpm dependencies (local har) ..."
(cd "$SCRIPT_DIR/entry" && ohpm install)

echo "[3/3] Building example HAP ..."
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
