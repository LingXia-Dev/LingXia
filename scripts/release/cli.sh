#!/usr/bin/env bash
set -euo pipefail

START_DIR="$(pwd)"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
WORKSPACE_CARGO_TOML="$ROOT_DIR/Cargo.toml"
GH_REPO="${LINGXIA_RELEASE_REPO:-LingXia-Dev/LingXia}"
ALL_TARGETS=(darwin-x64 darwin-arm64)

usage() {
  cat <<'EOF'
Build and optionally publish LingXia CLI release assets.

Usage:
  scripts/release/cli.sh [OPTIONS]

Options:
  --target <platform>  Build specific target(s): darwin-x64, darwin-arm64, all
  --publish            Upload built assets to the GitHub release tag
  --tag <tag>          Release tag to upload to (default: lingxia-cli-v<version>)
  --out <dir>          Output directory (default: dist/cli-release)
  --skip-build         Reuse existing cargo artifacts
  -h, --help           Show help

Environment:
  LINGXIA_RELEASE_REPO  Override target repo (default: LingXia-Dev/LingXia)

Examples:
  scripts/release/cli.sh --target darwin-arm64
  scripts/release/cli.sh --target all
  scripts/release/cli.sh --publish
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

release_tag_for_version() {
  local version="$1"
  printf 'lingxia-cli-v%s\n' "$version"
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "ERROR: missing required command: $1" >&2
    exit 1
  }
}

current_cli_target() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Darwin) os="darwin" ;;
    *)
      echo "ERROR: unsupported CLI host OS: $os" >&2
      return 2
      ;;
  esac

  case "$arch" in
    x86_64|amd64) arch="x86_64" ;;
    arm64|aarch64) arch="aarch64" ;;
    *)
      echo "ERROR: unsupported CLI host arch: $arch" >&2
      return 2
      ;;
  esac

  printf '%s-%s\n' "$os" "$arch"
}

cli_target_info() {
  case "$1" in
    darwin-x64)   echo "x86_64-apple-darwin lingxia-darwin-x86_64" ;;
    darwin-arm64) echo "aarch64-apple-darwin lingxia-darwin-aarch64" ;;
    *) return 1 ;;
  esac
}

resolve_out_dir() {
  local default_dir="$1"
  local out_dir="${2:-$default_dir}"
  if [[ "$out_dir" != /* ]]; then
    out_dir="$START_DIR/$out_dir"
  fi
  printf '%s\n' "$out_dir"
}

ensure_github_release() {
  local tag="$1"
  local version="$2"

  require_command gh
  if gh release view "$tag" --repo "$GH_REPO" >/dev/null 2>&1; then
    return 0
  fi

  echo "==> Creating GitHub release $tag in $GH_REPO"
  gh release create "$tag" \
    --repo "$GH_REPO" \
    --title "$tag" \
    --notes "LingXia CLI release $version"
}

upload_github_release_asset() {
  local tag="$1"
  local asset_path="$2"
  gh release upload "$tag" "$asset_path" --repo "$GH_REPO" --clobber
}

if [[ $# -eq 0 ]]; then
  usage
  exit 2
fi

PUBLISH=0
OUT_DIR=""
SKIP_BUILD=0
TARGET=""
TAG=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --publish) PUBLISH=1 ;;
    --out) OUT_DIR="$2"; shift ;;
    --skip-build) SKIP_BUILD=1 ;;
    --target) TARGET="$2"; shift ;;
    --tag) TAG="$2"; shift ;;
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

VERSION="$(read_workspace_version)"
if [[ -z "$VERSION" ]]; then
  echo "ERROR: failed to read workspace version from $WORKSPACE_CARGO_TOML" >&2
  exit 1
fi

TAG="${TAG:-$(release_tag_for_version "$VERSION")}"
OUT_DIR="$(resolve_out_dir "$START_DIR/dist/cli-release" "$OUT_DIR")"
mkdir -p "$OUT_DIR"

if [[ -z "$TARGET" ]]; then
  if [[ "$PUBLISH" -eq 1 && "$SKIP_BUILD" -eq 0 ]]; then
    TARGET="all"
  else
    TARGET="$(current_cli_target)" || exit $?
  fi
fi

if [[ "$TARGET" == "all" ]]; then
  TARGETS=("${ALL_TARGETS[@]}")
else
  cli_target_info "$TARGET" >/dev/null || {
    echo "ERROR: unsupported --target '$TARGET'" >&2
    exit 2
  }
  TARGETS=("$TARGET")
fi

if [[ "$PUBLISH" -eq 1 ]]; then
  ensure_github_release "$TAG" "$VERSION"
fi

BUILT_ASSETS=()

for platform in "${TARGETS[@]}"; do
  read -r rust_target asset_name <<< "$(cli_target_info "$platform")"

  echo ""
  echo "========================================"
  echo "[cli:$platform] Building v$VERSION"
  echo "========================================"

  if [[ "$SKIP_BUILD" -eq 0 ]]; then
    (cd "$ROOT_DIR" && cargo build -p lingxia-cli --release --target "$rust_target")
  fi

  bin_src="$ROOT_DIR/target/$rust_target/release/lingxia"
  asset_out="$OUT_DIR/$asset_name"
  [[ -f "$bin_src" ]] || {
    echo "ERROR: CLI binary not found: $bin_src" >&2
    exit 1
  }

  cp "$bin_src" "$asset_out"
  chmod +x "$asset_out"
  BUILT_ASSETS+=("$asset_out")
  echo "✅ CLI asset -> $asset_out"
done

if [[ "$PUBLISH" -eq 1 ]]; then
  for asset_path in "${BUILT_ASSETS[@]}"; do
    upload_github_release_asset "$TAG" "$asset_path"
    echo "✅ Uploaded CLI asset $(basename "$asset_path") to $TAG"
  done
fi

echo ""
echo "✅ CLI release flow complete"
echo "   Targets: ${TARGETS[*]}"
echo "   Output:  $OUT_DIR"
if [[ "$PUBLISH" -eq 1 ]]; then
  echo "   Release: $TAG ($GH_REPO)"
fi
