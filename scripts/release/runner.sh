#!/usr/bin/env bash
set -euo pipefail

START_DIR="$(pwd)"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORKSPACE_CARGO_TOML="$ROOT_DIR/Cargo.toml"
RUNNER_RAW_APP_DIR="$ROOT_DIR/tools/lingxia-runner/.lingxia"
RUNNER_RAW_DIST_DIR="$ROOT_DIR/tools/lingxia-runner/dist/macos"
RUNNER_RELEASE_APP_NAME="LingXia Runner.app"
GH_REPO="${LINGXIA_RELEASE_REPO:-LingXia-Dev/LingXia}"

usage() {
  cat <<'EOF'
Usage: scripts/release/runner.sh [OPTIONS]

Options:
  --publish           Upload the Runner zip to GitHub Release
  --tag <tag>         Release tag to upload to (default: lingxia-cli-v<version>)
  --out <dir>         Output directory (default: dist/runner-release)
  --macos-arch <arch> Build a specific macOS arch: arm64 or x86_64
  --skip-build        Reuse existing lingxia build artifacts from tools/lingxia-runner/.lingxia and dist/macos

Environment:
  LINGXIA_RELEASE_REPO  Override target repo (default: LingXia-Dev/LingXia)

Examples:
  scripts/release/runner.sh
  scripts/release/runner.sh --macos-arch arm64
  scripts/release/runner.sh --publish
  scripts/release/runner.sh --out /tmp/runner-release
EOF
}

read_workspace_version() {
  awk '
    /^\[workspace\.package\]/ {in_section=1; next}
    /^\[/ {in_section=0}
    in_section && $1 == "version" {
      gsub(/"/, "", $3);
      print $3;
      exit
    }' "$WORKSPACE_CARGO_TOML"
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "ERROR: missing required command: $1" >&2
    exit 1
  }
}

ensure_macos() {
  if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "ERROR: LingXia Runner release is macOS-only." >&2
    exit 1
  fi
}

default_macos_arch() {
  case "$(uname -m)" in
    arm64|aarch64) echo "arm64" ;;
    x86_64|amd64) echo "x86_64" ;;
    *)
      echo "ERROR: unsupported host architecture: $(uname -m)" >&2
      exit 2
      ;;
  esac
}

runner_arch_suffix() {
  case "$1" in
    arm64) echo "arm64" ;;
    x86_64) echo "x64" ;;
    *)
      echo "ERROR: unsupported Runner arch: $1" >&2
      exit 2
      ;;
  esac
}

PUBLISH=0
OUT_DIR=""
SKIP_BUILD=0
TAG=""
MACOS_ARCH=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --publish) PUBLISH=1 ;;
    --tag) TAG="$2"; shift ;;
    --out) OUT_DIR="$2"; shift ;;
    --macos-arch) MACOS_ARCH="$2"; shift ;;
    --skip-build) SKIP_BUILD=1 ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage
      exit 2
      ;;
  esac
  shift
done

ensure_macos
require_command cargo

VERSION="$(read_workspace_version)"
if [[ -z "$VERSION" ]]; then
  echo "ERROR: failed to read workspace version from $WORKSPACE_CARGO_TOML" >&2
  exit 1
fi

TAG="${TAG:-lingxia-cli-v$VERSION}"
OUT_DIR="${OUT_DIR:-$START_DIR/dist/runner-release}"
[[ "$OUT_DIR" != /* ]] && OUT_DIR="$START_DIR/$OUT_DIR"
mkdir -p "$OUT_DIR"

if [[ -n "$MACOS_ARCH" && "$MACOS_ARCH" != "arm64" && "$MACOS_ARCH" != "x86_64" ]]; then
  echo "ERROR: unsupported --macos-arch '$MACOS_ARCH' (expected arm64 or x86_64)" >&2
  exit 2
fi

EFFECTIVE_MACOS_ARCH="${MACOS_ARCH:-$(default_macos_arch)}"
ARCH_SUFFIX="$(runner_arch_suffix "$EFFECTIVE_MACOS_ARCH")"

RAW_APP_SRC="$RUNNER_RAW_APP_DIR/LingXia Runner.app"
RAW_ZIP_SRC="$RUNNER_RAW_DIST_DIR/LingXia Runner-$VERSION-macos.zip"
APP_OUT="$OUT_DIR/LingXia Runner-$ARCH_SUFFIX.app"
ZIP_OUT="$OUT_DIR/lingxia-runner-$VERSION-macos-$ARCH_SUFFIX.zip"

if [[ "$SKIP_BUILD" -ne 1 ]]; then
  echo "==> Building LingXia Runner release"
  cargo build --manifest-path "$ROOT_DIR/tools/lingxia-cli/Cargo.toml" -p lingxia-cli
  CLI_BIN="$ROOT_DIR/target/debug/lingxia"
  [[ -x "$CLI_BIN" ]] || {
    echo "ERROR: missing built CLI binary: $CLI_BIN" >&2
    exit 1
  }
  (
    cd "$ROOT_DIR/tools/lingxia-runner"
    build_cmd=(
      "$CLI_BIN"
      build --platform macos --package --release
    )
    if [[ -n "$MACOS_ARCH" ]]; then
      build_cmd+=(--macos-arch "$MACOS_ARCH")
    fi
    "${build_cmd[@]}"
  )
fi

[[ -d "$RAW_APP_SRC" ]] || {
  echo "ERROR: missing Runner app bundle from lingxia build: $RAW_APP_SRC" >&2
  exit 1
}
[[ -f "$RAW_ZIP_SRC" ]] || {
  echo "ERROR: missing Runner release zip from lingxia build: $RAW_ZIP_SRC" >&2
  exit 1
}

rm -rf "$APP_OUT"
cp -R "$RAW_APP_SRC" "$APP_OUT"
cp "$RAW_ZIP_SRC" "$ZIP_OUT"

echo "✅ Runner app  -> $APP_OUT"
echo "✅ Runner zip  -> $ZIP_OUT"

if [[ "$PUBLISH" -eq 1 ]]; then
  require_command gh

  if gh release view "$TAG" --repo "$GH_REPO" >/dev/null 2>&1; then
    echo "==> Uploading Runner asset to existing release $TAG ($GH_REPO)"
  else
    echo "==> Creating release $TAG in $GH_REPO"
    gh release create "$TAG" \
      --repo "$GH_REPO" \
      --title "$TAG" \
      --notes "LingXia Runner release $VERSION"
  fi

  gh release upload "$TAG" "$ZIP_OUT" --repo "$GH_REPO" --clobber
  echo "✅ Uploaded Runner zip to GitHub release $TAG ($GH_REPO)"
fi
