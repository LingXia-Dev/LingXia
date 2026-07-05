#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 0 ]]; then
  echo "Usage: $0" >&2
  echo "Environment: RUNNER_VERSION, RUNNER_TARGET_DIR, LINGXIA_BIN, CARGO_BIN, NPM_BIN" >&2
  exit 2
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../../.." && pwd)"
# The Runner is versioned with the CLI (tools version), independent of the SDK
# crates' [workspace.package] version — so read the Runner's own package version.
RUNNER_CARGO_TOML="$SCRIPT_DIR/native/Cargo.toml"

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
APP_NAME="LingXia Runner.app"
BRIDGE_DIR="$ROOT_DIR/packages/lingxia-bridge"
BRIDGE_RUNTIME="$BRIDGE_DIR/dist/bridge-runtime.es2020.js"

case "$TARGET_DIR" in
  "$HOME/.lingxia/runner/"*) ;;
  *)
    echo "ERROR: refusing to clear non-runner directory: $TARGET_DIR" >&2
    exit 2
    ;;
esac

if ! command -v "$CARGO_BIN" >/dev/null 2>&1; then
  echo "ERROR: missing cargo: $CARGO_BIN" >&2
  exit 1
fi

if ! command -v "$NPM_BIN" >/dev/null 2>&1; then
  echo "ERROR: missing npm: $NPM_BIN" >&2
  exit 1
fi

# Build + use the CLI from this repo so the Runner is built by the matching
# toolchain and `--with-provider` (LINGXIA_WITH_PROVIDERS) injection applies.
# shellcheck source=/dev/null
source "$ROOT_DIR/scripts/lib/lingxia.sh"
ensure_lingxia "$ROOT_DIR"
# shellcheck source=../../../scripts/lib/cargo-target-dir.sh
source "$ROOT_DIR/scripts/lib/cargo-target-dir.sh"
TARGET_BASE="$(resolve_cargo_target_dir "$SCRIPT_DIR" "$ROOT_DIR/Cargo.toml")"
APP_SRC="$TARGET_BASE/lingxia/macos/$APP_NAME"

echo "==> Cleaning Runner build outputs"
rm -rf "$SCRIPT_DIR/.build" "$SCRIPT_DIR/.lingxia" "$TARGET_BASE/lingxia/macos"

echo "==> Building bridge runtime"
(
  cd "$BRIDGE_DIR"
  "$NPM_BIN" run build
)

if [[ ! -f "$BRIDGE_RUNTIME" ]]; then
  echo "ERROR: bridge runtime not found after build: $BRIDGE_RUNTIME" >&2
  exit 1
fi

# Stage it before the Swift build: the build plugin syncs it too late for
# SwiftPM's plan-time `.copy("Resources")`, so on a clean checkout it would be
# absent from the bundle and lx://assets/bridge-runtime.js would 404.
cp "$BRIDGE_RUNTIME" "$SCRIPT_DIR/Sources/Resources/bridge-runtime.js"

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
# Standalone Swift Package: one `lingxia build` runs the Swift build whose plugin
# compiles the native lib, picking up LINGXIA_WITH_PROVIDERS injection.
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
