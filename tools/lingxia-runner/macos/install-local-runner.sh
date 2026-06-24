#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 0 ]]; then
  echo "Usage: $0" >&2
  echo "Environment: RUNNER_VERSION, RUNNER_TARGET_DIR, LINGXIA_BIN, CARGO_BIN, NPM_BIN, MACOS_ARCH" >&2
  exit 2
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../../.." && pwd)"
# The Runner is versioned with the CLI (tools version), independent of the SDK
# crates' [workspace.package] version — so read the Runner's own package version.
RUNNER_CARGO_TOML="$SCRIPT_DIR/runner-lib/Cargo.toml"

read_tools_version() {
  awk -F'"' '/^version = "/ { print $2; exit }' "$RUNNER_CARGO_TOML"
}

RUNNER_VERSION="${RUNNER_VERSION:-$(read_tools_version)}"
if [[ -z "$RUNNER_VERSION" ]]; then
  echo "ERROR: failed to read runner (tools) version from $RUNNER_CARGO_TOML" >&2
  exit 1
fi
TARGET_DIR="${RUNNER_TARGET_DIR:-$HOME/.lingxia/runner/$RUNNER_VERSION}"
TARGET_PARENT="$(dirname "$TARGET_DIR")"
TMP_TARGET_DIR="$TARGET_PARENT/.tmp-runner-$RUNNER_VERSION-$$"
BACKUP_TARGET_DIR="$TARGET_PARENT/.prev-runner-$RUNNER_VERSION-$$"
LINGXIA_BIN="${LINGXIA_BIN:-lingxia}"
CARGO_BIN="${CARGO_BIN:-cargo}"
NPM_BIN="${NPM_BIN:-npm}"
MACOS_ARCH="${MACOS_ARCH:-$(uname -m)}"
APP_NAME="LingXia Runner.app"
APP_SRC="$SCRIPT_DIR/.lingxia/$APP_NAME"
BRIDGE_DIR="$ROOT_DIR/packages/lingxia-bridge"
BRIDGE_RUNTIME="$BRIDGE_DIR/dist/bridge-runtime.es2020.js"

case "$MACOS_ARCH" in
  arm64|aarch64) RUST_TARGET="aarch64-apple-darwin" ;;
  x86_64|amd64) RUST_TARGET="x86_64-apple-darwin" ;;
  *)
    echo "ERROR: unsupported macOS architecture: $MACOS_ARCH" >&2
    exit 2
    ;;
esac

case "$TARGET_DIR" in
  "$HOME/.lingxia/runner/"*) ;;
  *)
    echo "ERROR: refusing to clear non-runner directory: $TARGET_DIR" >&2
    exit 2
    ;;
esac

if ! command -v "$LINGXIA_BIN" >/dev/null 2>&1; then
  echo "ERROR: missing lingxia CLI: $LINGXIA_BIN" >&2
  echo "Set LINGXIA_BIN=/path/to/lingxia if it is not on PATH." >&2
  exit 1
fi

if ! command -v "$CARGO_BIN" >/dev/null 2>&1; then
  echo "ERROR: missing cargo: $CARGO_BIN" >&2
  exit 1
fi

if ! command -v "$NPM_BIN" >/dev/null 2>&1; then
  echo "ERROR: missing npm: $NPM_BIN" >&2
  exit 1
fi

echo "==> Cleaning Runner build outputs"
rm -rf "$SCRIPT_DIR/.build" "$SCRIPT_DIR/.lingxia"

echo "==> Building bridge runtime"
(
  cd "$BRIDGE_DIR"
  "$NPM_BIN" run build
)

if [[ ! -f "$BRIDGE_RUNTIME" ]]; then
  echo "ERROR: bridge runtime not found after build: $BRIDGE_RUNTIME" >&2
  exit 1
fi

echo "==> Building Runner native staticlib ($RUST_TARGET)"
(
  cd "$ROOT_DIR"
  "$CARGO_BIN" build -p lingxia-runner-lib --lib --target "$RUST_TARGET"
)

echo "==> Generating apple SDK resources (i18n + icons)"
# Same step bootstrap-apple-sdk / scripts/release/sdk.sh run: without it a new or
# changed design/icons/svg or i18n YAML never reaches the runner bundle.
(
  cd "$ROOT_DIR"
  "$LINGXIA_BIN" gen i18n \
    --input i18n --no-rust --no-ts --no-android --no-harmony \
    --ios-out lingxia-sdk/apple/Sources/Resources
  "$LINGXIA_BIN" gen icons \
    --input design/icons/svg \
    --ios-out lingxia-sdk/apple/Sources/Resources/icons
)

echo "==> Building Runner"
(
  cd "$SCRIPT_DIR"
  "$LINGXIA_BIN" build
)

if [[ ! -d "$APP_SRC" ]]; then
  echo "ERROR: built app not found: $APP_SRC" >&2
  exit 1
fi

echo "==> Installing $APP_NAME to $TARGET_DIR"
rm -rf "$TMP_TARGET_DIR" "$BACKUP_TARGET_DIR"
mkdir -p "$TMP_TARGET_DIR"
cp -R "$APP_SRC" "$TMP_TARGET_DIR/"
mkdir -p "$TARGET_PARENT"
if [[ -e "$TARGET_DIR" ]]; then
  mv "$TARGET_DIR" "$BACKUP_TARGET_DIR"
fi
mv "$TMP_TARGET_DIR" "$TARGET_DIR"
rm -rf "$BACKUP_TARGET_DIR"

echo "Done: $TARGET_DIR/$APP_NAME"
