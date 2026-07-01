#!/usr/bin/env bash
set -euo pipefail

START_DIR="$(pwd)"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CLI_CARGO_TOML="$ROOT_DIR/tools/lingxia-cli/Cargo.toml"
RUNNER_PACKAGE_DIR="$ROOT_DIR/tools/lingxia-runner/macos"
RUNNER_RAW_APP_DIR="$ROOT_DIR/tools/lingxia-runner/macos/.lingxia"
RUNNER_RAW_DIST_DIR="$ROOT_DIR/tools/lingxia-runner/macos/dist/macos"
RUNNER_RELEASE_APP_NAME="LingXia Runner.app"
GH_REPO="${LINGXIA_RELEASE_REPO:-LingXia-Dev/LingXia}"

usage() {
  cat <<'EOF'
Usage: scripts/release/runner.sh [OPTIONS]

Options:
  --publish           Upload the Runner zip to GitHub Release
  --tag <tag>         Release tag to upload to (default: lingxia-cli-v<version>)
  --out <dir>         Output directory (default: dist/runner-release)
  --macos-arch <arch> Build a specific macOS arch: arm64, x86_64, or all
  --skip-build        Reuse existing lingxia build artifacts from tools/lingxia-runner/macos/.lingxia and dist/macos

Environment:
  LINGXIA_RELEASE_REPO  Override target repo (default: LingXia-Dev/LingXia)

Examples:
  scripts/release/runner.sh --macos-arch arm64
  scripts/release/runner.sh --macos-arch all
  scripts/release/runner.sh --publish
  scripts/release/runner.sh --out /tmp/runner-release
EOF
}

read_cli_version() {
  awk '
    /^\[package\]/ {in_section=1; next}
    /^\[/ {in_section=0}
    in_section && $1 == "version" {
      gsub(/"/, "", $3);
      print $3;
      exit
    }' "$CLI_CARGO_TOML"
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

release_tag_for_version() {
  local version="$1"
  printf 'lingxia-cli-v%s\n' "$version"
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

  echo "==> Creating release $tag in $GH_REPO"
  gh release create "$tag" \
    --repo "$GH_REPO" \
    --title "$tag" \
    --notes "LingXia Runner release $version"
}

if [[ $# -eq 0 ]]; then
  usage
  exit 2
fi

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

VERSION="$(read_cli_version)"
if [[ -z "$VERSION" ]]; then
  echo "ERROR: failed to read lingxia-cli version from $CLI_CARGO_TOML" >&2
  exit 1
fi

TAG="${TAG:-$(release_tag_for_version "$VERSION")}"
OUT_DIR="$(resolve_out_dir "$START_DIR/dist/runner-release" "$OUT_DIR")"
mkdir -p "$OUT_DIR"

if [[ -z "$MACOS_ARCH" ]]; then
  if [[ "$PUBLISH" -eq 1 && "$SKIP_BUILD" -eq 0 ]]; then
    MACOS_ARCH="all"
  else
    MACOS_ARCH="$(default_macos_arch)"
  fi
fi

case "$MACOS_ARCH" in
  arm64|x86_64) ARCHES=("$MACOS_ARCH") ;;
  all) ARCHES=(arm64 x86_64) ;;
  *)
    echo "ERROR: unsupported --macos-arch '$MACOS_ARCH' (expected arm64, x86_64, or all)" >&2
    exit 2
    ;;
esac

if [[ "$SKIP_BUILD" -eq 1 && "${#ARCHES[@]}" -gt 1 ]]; then
  echo "ERROR: --skip-build cannot be used with --macos-arch all." >&2
  echo "       Raw runner outputs are single-arch and would be reused incorrectly." >&2
  exit 2
fi

if [[ "$SKIP_BUILD" -ne 1 ]]; then
  cargo build --manifest-path "$ROOT_DIR/tools/lingxia-cli/Cargo.toml" -p lingxia-cli
fi

CLI_BIN="$ROOT_DIR/target/debug/lingxia"
if [[ "$SKIP_BUILD" -ne 1 ]]; then
  [[ -x "$CLI_BIN" ]] || {
    echo "ERROR: missing built CLI binary: $CLI_BIN" >&2
    exit 1
  }
fi

if [[ "$PUBLISH" -eq 1 ]]; then
  ensure_github_release "$TAG" "$VERSION"
fi

BUILT_ZIPS=()

for arch in "${ARCHES[@]}"; do
  ARCH_SUFFIX="$(runner_arch_suffix "$arch")"
  RAW_APP_SRC="$RUNNER_RAW_APP_DIR/LingXia Runner.app"
  RAW_ZIP_SRC="$RUNNER_RAW_DIST_DIR/LingXia Runner-$VERSION-macos.zip"
  APP_OUT="$OUT_DIR/LingXia Runner-$ARCH_SUFFIX.app"
  ZIP_OUT="$OUT_DIR/lingxia-runner-$VERSION-macos-$ARCH_SUFFIX.zip"

  echo ""
  echo "========================================"
  echo "[runner:$arch] Building v$VERSION"
  echo "========================================"

  if [[ "$SKIP_BUILD" -ne 1 ]]; then
    echo "[runner:$arch] Cleaning Runner build outputs"
    # Also wipe the raw dist dir: $RAW_ZIP_SRC lives there, and a stale zip from
    # the previous arch would otherwise be copied into this arch's output,
    # silently shipping the wrong architecture under the right name.
    rm -rf "$RUNNER_PACKAGE_DIR/.build" "$RUNNER_PACKAGE_DIR/.lingxia" "$RUNNER_RAW_DIST_DIR"
    (
      cd "$RUNNER_PACKAGE_DIR"
      "$CLI_BIN" package --platform macos --macos-arch "$arch"
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

  # Verify the built binary is actually the requested arch. This is host-agnostic
  # (a correct cross-build passes; only a misconfigured one fails) and catches a
  # silent mislabel — e.g. an x86_64 binary packaged into an arm64-named zip when
  # cross-compilation is not wired up on this host ($(uname -m)).
  require_command lipo
  runner_exe="$(find "$RAW_APP_SRC/Contents/MacOS" -maxdepth 1 -type f -perm -u+x 2>/dev/null | head -n1)"
  [[ -n "$runner_exe" ]] || {
    echo "ERROR: no executable found in $RAW_APP_SRC/Contents/MacOS" >&2
    exit 1
  }
  built_archs="$(lipo -archs "$runner_exe" 2>/dev/null || true)"
  if [[ " $built_archs " != *" $arch "* ]]; then
    echo "ERROR: Runner binary arch mismatch: requested '$arch', built '$built_archs'." >&2
    echo "       Cross-compilation for '$arch' may not be configured on this host ($(uname -m))." >&2
    exit 1
  fi
  echo "[runner:$arch] Verified binary arch: $built_archs"

  # The Runner is a SwiftPM executable, so its generated `Bundle.module`
  # accessor resolves resources via `Bundle.main.bundleURL` — which, once the
  # binary is wrapped in a `.app`, is the `.app` root, NOT `Contents/Resources`
  # where SwiftPM's module bundles actually land. A fetched release Runner (its
  # baked `.build` fallback path gone) therefore fatal-errors on launch:
  #   "could not load resource bundle: from <app>/LingXiaRunner_LingXiaRunner.bundle …"
  # Mirror the SwiftPM module bundles (`<Pkg>_<Target>.bundle`) to the `.app`
  # root so the accessor finds them. This runs AFTER `lingxia package` has
  # signed + notarized + verified the clean bundle, and `lingxia dev`
  # direct-execs the (still validly signed) binary — so the extra root copies
  # don't affect launch. (Gatekeeper `open` would see an unsealed root, but the
  # fetched Runner is only ever direct-exec'd, never opened.)
  shopt -s nullglob
  for _b in "$RAW_APP_SRC/Contents/Resources/"*_*.bundle; do
    rm -rf "$RAW_APP_SRC/$(basename "$_b")"
    cp -R "$_b" "$RAW_APP_SRC/$(basename "$_b")"
  done
  shopt -u nullglob

  rm -rf "$APP_OUT"
  cp -R "$RAW_APP_SRC" "$APP_OUT"

  # Re-zip the fixed `.app` (keeping the canonical "LingXia Runner.app" name
  # that runner_cache expects) rather than shipping lingxia package's pre-fix zip.
  require_command ditto
  rm -f "$ZIP_OUT"
  ditto -c -k --keepParent "$RAW_APP_SRC" "$ZIP_OUT"
  BUILT_ZIPS+=("$ZIP_OUT")

  echo "✅ Runner app  -> $APP_OUT"
  echo "✅ Runner zip  -> $ZIP_OUT"
done

if [[ "$PUBLISH" -eq 1 ]]; then
  for zip_path in "${BUILT_ZIPS[@]}"; do
    gh release upload "$TAG" "$zip_path" --repo "$GH_REPO" --clobber
    echo "✅ Uploaded Runner zip $(basename "$zip_path") to GitHub release $TAG ($GH_REPO)"
  done
fi

echo ""
echo "✅ Runner release flow complete"
echo "   Arches:  ${ARCHES[*]}"
echo "   Output:  $OUT_DIR"
if [[ "$PUBLISH" -eq 1 ]]; then
  echo "   Release: $TAG ($GH_REPO)"
fi
