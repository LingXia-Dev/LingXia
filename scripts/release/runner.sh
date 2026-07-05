#!/usr/bin/env bash
set -euo pipefail

START_DIR="$(pwd)"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CLI_CARGO_TOML="$ROOT_DIR/tools/lingxia-cli/Cargo.toml"
RUNNER_PACKAGE_DIR="$ROOT_DIR/tools/lingxia-runner/macos"
# shellcheck source=../lib/cargo-target-dir.sh
source "$ROOT_DIR/scripts/lib/cargo-target-dir.sh"
RUNNER_RAW_APP_DIR=""
RUNNER_RAW_DIST_DIR="$ROOT_DIR/tools/lingxia-runner/macos/dist/macos"
RUNNER_RELEASE_APP_NAME="LingXia Runner.app"
RUNNER_WINDOWS_DIR="$ROOT_DIR/tools/lingxia-runner/windows"
RUNNER_WINDOWS_ASSET_NAME="lingxia-runner-windows-x64.zip"
GH_REPO="${LINGXIA_RELEASE_REPO:-LingXia-Dev/LingXia}"

usage() {
  cat <<'EOF'
Usage: scripts/release/runner.sh [OPTIONS]

Options:
  --publish           Upload the Runner zip to GitHub Release
  --tag <tag>         Release tag to upload to (default: lingxia-cli-v<version>)
  --out <dir>         Output directory (default: dist/runner-release)
  --platform <name>   Runner platform to build: macos or windows (default: current host)
  --macos-arch <arch> Build a specific macOS arch: arm64, x86_64, or all
  --skip-build        Reuse existing lingxia build artifacts from target/lingxia/macos and dist/macos

Environment:
  LINGXIA_RELEASE_REPO  Override target repo (default: LingXia-Dev/LingXia)

Examples:
  scripts/release/runner.sh --macos-arch arm64
  scripts/release/runner.sh --macos-arch all
  scripts/release/runner.sh --platform windows
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
      gsub(/\r/, "", $3);
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

sha256_one() {
  local f="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$f" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$f" | awk '{print $1}'
  else
    echo "ERROR: missing sha256sum or shasum" >&2
    exit 1
  fi
}

ensure_macos() {
  if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "ERROR: LingXia Runner release is macOS-only." >&2
    exit 1
  fi
}

ensure_windows() {
  case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*|Windows_NT) ;;
    Linux)
      if command -v powershell.exe >/dev/null 2>&1 && command -v wslpath >/dev/null 2>&1; then
        return 0
      fi
      echo "ERROR: Windows LingXia Runner release must run on Windows or WSL with powershell.exe." >&2
      exit 1
      ;;
    *)
      echo "ERROR: Windows LingXia Runner release must run on Windows." >&2
      exit 1
      ;;
  esac
}

current_runner_platform() {
  case "$(uname -s)" in
    Darwin) echo "macos" ;;
    MINGW*|MSYS*|CYGWIN*|Windows_NT) echo "windows" ;;
    Linux)
      if command -v powershell.exe >/dev/null 2>&1 && command -v wslpath >/dev/null 2>&1; then
        echo "windows"
      else
        echo "ERROR: unsupported Runner release host: $(uname -s)" >&2
        exit 2
      fi
      ;;
    *)
      echo "ERROR: unsupported Runner release host: $(uname -s)" >&2
      exit 2
      ;;
  esac
}

powershell_bin() {
  if command -v pwsh >/dev/null 2>&1; then
    echo "pwsh"
  elif command -v powershell.exe >/dev/null 2>&1; then
    echo "powershell.exe"
  elif command -v powershell >/dev/null 2>&1; then
    echo "powershell"
  else
    echo "ERROR: missing PowerShell (pwsh or powershell.exe)" >&2
    exit 1
  fi
}

windows_path() {
  if command -v cygpath >/dev/null 2>&1; then
    cygpath -w "$1"
  elif command -v wslpath >/dev/null 2>&1; then
    wslpath -w "$1"
  else
    printf '%s\n' "$1"
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

update_release_shasums() {
  local shasums_name="SHASUMS256-$VERSION.txt"
  local tmp_dir="$OUT_DIR/.runner-shasums"
  local current_file="$tmp_dir/current"
  local next_file="$tmp_dir/next"

  rm -rf "$tmp_dir"
  mkdir -p "$tmp_dir"

  if gh release download "$TAG" --repo "$GH_REPO" --pattern "$shasums_name" --dir "$tmp_dir" >/dev/null 2>&1 \
    && [[ -f "$tmp_dir/$shasums_name" ]]; then
    cp "$tmp_dir/$shasums_name" "$current_file"
  else
    : > "$current_file"
  fi

  cp "$current_file" "$next_file"
  for zip_path in "${BUILT_ZIPS[@]}"; do
    local asset_name asset_sha filtered_file
    asset_name="$(basename "$zip_path")"
    asset_sha="$(sha256_one "$zip_path")"
    filtered_file="$tmp_dir/filtered"

    awk -v name="$asset_name" '
      {
        file = $NF
        sub(/^\*/, "", file)
        if (file != name) print
      }
    ' "$next_file" > "$filtered_file"
    mv "$filtered_file" "$next_file"
    printf '%s  %s\n' "$asset_sha" "$asset_name" >> "$next_file"
  done

  sort -k2 "$next_file" > "$tmp_dir/$shasums_name"
  gh release upload "$TAG" "$tmp_dir/$shasums_name" --repo "$GH_REPO" --clobber
  echo "✅ Uploaded $shasums_name to GitHub release $TAG ($GH_REPO)"
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
PLATFORM=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --publish) PUBLISH=1 ;;
    --tag) TAG="$2"; shift ;;
    --out) OUT_DIR="$2"; shift ;;
    --platform) PLATFORM="$2"; shift ;;
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

VERSION="$(read_cli_version)"
if [[ -z "$VERSION" ]]; then
  echo "ERROR: failed to read lingxia-cli version from $CLI_CARGO_TOML" >&2
  exit 1
fi

TAG="${TAG:-$(release_tag_for_version "$VERSION")}"
OUT_DIR="$(resolve_out_dir "$START_DIR/dist/runner-release" "$OUT_DIR")"
mkdir -p "$OUT_DIR"

PLATFORM="${PLATFORM:-$(current_runner_platform)}"
case "$PLATFORM" in
  macos) BUILD_MACOS=1; BUILD_WINDOWS=0 ;;
  windows) BUILD_MACOS=0; BUILD_WINDOWS=1 ;;
  *)
    echo "ERROR: unsupported --platform '$PLATFORM' (expected macos or windows)" >&2
    exit 2
    ;;
esac

if [[ "$BUILD_MACOS" -eq 1 ]]; then
  ensure_macos
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
else
  ensure_windows
  ARCHES=()
fi

if [[ "$BUILD_MACOS" -eq 1 && "$SKIP_BUILD" -ne 1 ]]; then
  require_command cargo
  ( cd "$ROOT_DIR" && cargo build --manifest-path "$ROOT_DIR/tools/lingxia-cli/Cargo.toml" -p lingxia-cli )
fi

if [[ "$BUILD_MACOS" -eq 1 ]]; then
  RUNNER_TARGET_DIR="$(
    resolve_cargo_target_dir "$RUNNER_PACKAGE_DIR" "$ROOT_DIR/Cargo.toml"
  )"
  RUNNER_RAW_APP_DIR="$RUNNER_TARGET_DIR/lingxia/macos"
fi

CLI_BIN=""
if [[ "$BUILD_MACOS" -eq 1 ]]; then
  CLI_TARGET_DIR="$(resolve_cargo_target_dir "$ROOT_DIR" "$ROOT_DIR/Cargo.toml")"
  CLI_BIN="$CLI_TARGET_DIR/debug/lingxia"
  if [[ "$SKIP_BUILD" -ne 1 ]]; then
    [[ -x "$CLI_BIN" ]] || {
      echo "ERROR: missing built CLI binary: $CLI_BIN" >&2
      exit 1
    }
  fi
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
    rm -rf "$RUNNER_PACKAGE_DIR/.build" "$RUNNER_PACKAGE_DIR/.lingxia" "$RUNNER_RAW_APP_DIR" "$RUNNER_RAW_DIST_DIR"

    # Sources/Resources/bridge-runtime.js is gitignored and the build-tool plugin
    # writes it too late for SwiftPM's plan-time `.copy("Resources")` on a clean
    # checkout — so a released Runner would ship without it and every page's
    # `lx://assets/bridge-runtime.js` would 404 (dead bridge). Stage it up front.
    BRIDGE_DIST="$ROOT_DIR/packages/lingxia-bridge/dist/bridge-runtime.es2020.js"
    if [[ ! -f "$BRIDGE_DIST" ]]; then
      echo "[runner:$arch] Building bridge runtime (packages/lingxia-bridge)"
      ( cd "$ROOT_DIR/packages/lingxia-bridge" && npm install && npm run build )
    fi
    cp "$BRIDGE_DIST" "$RUNNER_PACKAGE_DIR/Sources/Resources/bridge-runtime.js"

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

  rm -rf "$APP_OUT"
  cp -R "$RAW_APP_SRC" "$APP_OUT"
  cp "$RAW_ZIP_SRC" "$ZIP_OUT"
  BUILT_ZIPS+=("$ZIP_OUT")

  echo "✅ Runner app  -> $APP_OUT"
  echo "✅ Runner zip  -> $ZIP_OUT"
done

if [[ "$BUILD_WINDOWS" -eq 1 ]]; then
  ZIP_OUT="$OUT_DIR/$RUNNER_WINDOWS_ASSET_NAME"

  echo ""
  echo "========================================"
  echo "[runner:windows-x64] Building v$VERSION"
  echo "========================================"

  if [[ "$SKIP_BUILD" -ne 1 ]]; then
    rm -f "$ZIP_OUT"
    rm -rf "$OUT_DIR/windows-runner-stage"

    ps="$(powershell_bin)"
    runner_script_win="$(windows_path "$RUNNER_WINDOWS_DIR/install-local-runner.ps1")"
    RUNNER_BUILD_PROFILE=release "$ps" -NoProfile -ExecutionPolicy Bypass -File "$runner_script_win"

    stage="$OUT_DIR/windows-runner-stage"
    mkdir -p "$stage"
    stage_win="$(windows_path "$stage")"
    zip_win="$(windows_path "$ZIP_OUT")"
    "$ps" -NoProfile -Command \
      "\$runnerDir = Join-Path ([Environment]::GetFolderPath('UserProfile')) '.lingxia\\runner\\$VERSION'; Copy-Item -LiteralPath (Join-Path \$runnerDir 'lingxia-runner.exe') -Destination '$stage_win\\lingxia-runner.exe' -Force; Compress-Archive -Path '$stage_win\\lingxia-runner.exe' -DestinationPath '$zip_win' -Force"
  fi

  [[ -f "$ZIP_OUT" ]] || {
    echo "ERROR: missing Windows Runner release zip: $ZIP_OUT" >&2
    exit 1
  }

  BUILT_ZIPS+=("$ZIP_OUT")
  echo "鉁?Runner zip  -> $ZIP_OUT"
fi

if [[ "$PUBLISH" -eq 1 ]]; then
  for zip_path in "${BUILT_ZIPS[@]}"; do
    gh release upload "$TAG" "$zip_path" --repo "$GH_REPO" --clobber
    echo "✅ Uploaded Runner zip $(basename "$zip_path") to GitHub release $TAG ($GH_REPO)"
  done
  update_release_shasums
fi

echo ""
echo "✅ Runner release flow complete"
if [[ "$BUILD_MACOS" -eq 1 ]]; then
  echo "   Platform: macos"
  echo "   Arches:   ${ARCHES[*]}"
else
  echo "   Platform: windows"
  echo "   Arches:   x64"
fi
echo "   Output:  $OUT_DIR"
if [[ "$PUBLISH" -eq 1 ]]; then
  echo "   Release: $TAG ($GH_REPO)"
fi
