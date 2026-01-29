#!/usr/bin/env bash
set -euo pipefail

START_DIR="$(pwd)"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
MAIN_PKG="$ROOT_DIR/tools/lingxia-cli/npm/package.json"
CLI_CARGO_TOML="$ROOT_DIR/tools/lingxia-cli/Cargo.toml"

# Supported targets: PLATFORM_NAME -> "RUST_TARGET OS CPU BIN_EXT"
get_target_info() {
  case "$1" in
    darwin-x64)   echo "x86_64-apple-darwin darwin x64 " ;;
    darwin-arm64) echo "aarch64-apple-darwin darwin arm64 " ;;
    win32-x64)    echo "x86_64-pc-windows-gnu win32 x64 .exe" ;;
    win32-arm64)  echo "aarch64-pc-windows-gnu win32 arm64 .exe" ;;
    *) return 1 ;;
  esac
}

ALL_TARGETS="darwin-x64 darwin-arm64 win32-x64 win32-arm64"

# Detect current platform
detect_platform() {
  local arch os
  arch="$(uname -m)"
  os="$(uname -s)"
  case "$os" in
    Darwin)
      case "$arch" in
        x86_64) echo "darwin-x64" ;;
        arm64)  echo "darwin-arm64" ;;
        *) echo ""; return 1 ;;
      esac ;;
    MINGW*|MSYS*|CYGWIN*)
      case "$arch" in
        x86_64)  echo "win32-x64" ;;
        aarch64) echo "win32-arm64" ;;
        *) echo ""; return 1 ;;
      esac ;;
    *) echo ""; return 1 ;;
  esac
}

# Parse arguments
PUBLISH=0
OUT_DIR=""
SKIP_BUILD=0
TARGET=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --publish) PUBLISH=1 ;;
    --out) OUT_DIR="$2"; shift ;;
    --skip-build) SKIP_BUILD=1 ;;
    --target) TARGET="$2"; shift ;;
    -h|--help)
      cat <<EOF
Usage: release.sh [OPTIONS]

Options:
  --target <platform>  Build specific platform(s):
                       darwin-x64, darwin-arm64, win32-x64, win32-arm64, all
                       Default: current platform
  --publish            Publish platform package(s) + main @lingxia/cli
  --out <dir>          Output directory (default: ./dist)
  --skip-build         Skip cargo build, use existing binaries

Examples:
  ./release.sh                              # Build current platform
  ./release.sh --target darwin-x64 --publish  # Build & publish Intel Mac
  ./release.sh --target all --publish       # Full release
EOF
      exit 0 ;;
    *) echo "Unknown option: $1"; exit 1 ;;
  esac
  shift
done

# Read and validate versions
VERSION="$(node -p "require('$MAIN_PKG').version" 2>/dev/null)" || {
  echo "ERROR: Failed to read version from $MAIN_PKG"; exit 1
}

cargo_version="$(awk -F\" '/^version =/ {print $2; exit}' "$CLI_CARGO_TOML")"
if [[ "$cargo_version" != "$VERSION" ]]; then
  if [[ "$PUBLISH" -eq 1 ]]; then
    echo "ERROR: Version mismatch - Cargo.toml ($cargo_version) != package.json ($VERSION)"
    exit 1
  fi
  echo "Syncing Cargo.toml version: $cargo_version -> $VERSION"
  sed -i.bak "s/^version = \"[^\"]*\"/version = \"$VERSION\"/" "$CLI_CARGO_TOML" && rm -f "$CLI_CARGO_TOML.bak"
fi

# Validate optionalDependencies versions
bad_deps="$(node -e "
  const pkg = require('$MAIN_PKG');
  const bad = Object.entries(pkg.optionalDependencies || {})
    .filter(([k, v]) => k.startsWith('@lingxia/cli-') && v !== pkg.version);
  if (bad.length) console.log(bad.map(([k,v]) => k + '@' + v).join(', '));
")"
if [[ -n "$bad_deps" ]]; then
  if [[ "$PUBLISH" -eq 1 ]]; then
    echo "ERROR: optionalDependencies version mismatch: $bad_deps"
    echo "Expected version: $VERSION"
    exit 1
  fi
  echo "Syncing optionalDependencies to $VERSION"
  node -e "
    const fs = require('fs');
    const pkg = JSON.parse(fs.readFileSync('$MAIN_PKG'));
    for (const k of Object.keys(pkg.optionalDependencies || {})) {
      if (k.startsWith('@lingxia/cli-')) pkg.optionalDependencies[k] = pkg.version;
    }
    fs.writeFileSync('$MAIN_PKG', JSON.stringify(pkg, null, 2) + '\n');
  "
fi

# Setup output directory
OUT_DIR="${OUT_DIR:-$START_DIR/dist}"
[[ "$OUT_DIR" != /* ]] && OUT_DIR="$START_DIR/$OUT_DIR"
mkdir -p "$OUT_DIR"

# Build and optionally publish a single target
build_target() {
  local platform="$1"
  local info rust_target os cpu ext
  info="$(get_target_info "$platform")" || { echo "Unknown target: $platform"; return 1; }
  read -r rust_target os cpu ext <<< "$info"

  echo ""
  echo "========================================"
  echo "[$platform] Building v$VERSION"
  echo "========================================"

  if [[ "$SKIP_BUILD" -eq 0 ]]; then
    (cd "$ROOT_DIR" && cargo build -p lingxia-cli --release --target "$rust_target")
  fi

  local bin_src="$ROOT_DIR/target/$rust_target/release/lingxia$ext"
  [[ -f "$bin_src" ]] || { echo "ERROR: Binary not found: $bin_src"; return 1; }

  local pkg_dir="$OUT_DIR/$platform"
  mkdir -p "$pkg_dir/bin"
  cp "$bin_src" "$pkg_dir/bin/lingxia$ext"
  chmod +x "$pkg_dir/bin/lingxia$ext"

  cat > "$pkg_dir/package.json" <<EOF
{
  "name": "@lingxia/cli-$platform",
  "version": "$VERSION",
  "os": ["$os"],
  "cpu": ["$cpu"],
  "files": ["bin/lingxia$ext"],
  "license": "MIT"
}
EOF

  echo "✓ Package ready: $pkg_dir"

  if [[ "$PUBLISH" -eq 1 ]]; then
    if npm view "@lingxia/cli-$platform@$VERSION" version &>/dev/null; then
      echo "⚠ @lingxia/cli-$platform@$VERSION already published, skipping"
    else
      (cd "$pkg_dir" && npm publish --access public)
      echo "✓ Published @lingxia/cli-$platform@$VERSION"
    fi
  fi
}

# Determine which targets to build
if [[ "$TARGET" == "all" ]]; then
  targets=($ALL_TARGETS)
elif [[ -n "$TARGET" ]]; then
  get_target_info "$TARGET" >/dev/null || { echo "Unknown target: $TARGET"; exit 1; }
  targets=("$TARGET")
else
  detected="$(detect_platform)" || { echo "Unsupported platform"; exit 1; }
  targets=("$detected")
fi

# Build all targets
for t in "${targets[@]}"; do
  build_target "$t"
done

# Publish main package
if [[ "$PUBLISH" -eq 1 ]]; then
  echo ""
  echo "========================================"
  echo "Publishing @lingxia/cli@$VERSION"
  echo "========================================"
  if npm view "@lingxia/cli@$VERSION" version &>/dev/null; then
    echo "⚠ @lingxia/cli@$VERSION already published, skipping"
  else
    (cd "$ROOT_DIR/tools/lingxia-cli/npm" && npm publish --access public)
    echo "✓ Published @lingxia/cli@$VERSION"
  fi
fi

echo ""
echo "✅ Done! Built: ${targets[*]}"
echo "   Output: $OUT_DIR"
