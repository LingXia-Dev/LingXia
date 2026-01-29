#!/usr/bin/env bash
set -euo pipefail

START_DIR="$(pwd)"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
MAIN_PKG="$ROOT_DIR/tools/lingxia-cli/npm/package.json"

PUBLISH=0
OUT_DIR=""
CLEAN=1
SKIP_BUILD=0
TARGET=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --publish) PUBLISH=1 ;;
    --out) OUT_DIR="$2"; CLEAN=0; shift ;;
    --skip-build) SKIP_BUILD=1 ;;
    --target) TARGET="$2"; shift ;;
    -h|--help)
      echo "Usage: release.sh [--out <dir>] [--publish] [--skip-build] [--target <platform>]"
      echo ""
      echo "Options:"
      echo "  --target <platform>  Build specific platform or 'all' for all platforms"
      echo "                       Platforms: darwin-x64, darwin-arm64, win32-x64, win32-arm64, all"
      echo "  --publish            Build and publish all platforms"
      echo "  --out <dir>          Output directory (default: temp dir, or kept with --target)"
      echo "  --skip-build         Skip cargo build step"
      echo ""
      echo "Examples:"
      echo "  ./release.sh                        # Build current platform only"
      echo "  ./release.sh --target all --out dist  # Build all platforms to ./dist"
      echo "  ./release.sh --target darwin-arm64  # Build only Mac ARM"
      echo "  ./release.sh --publish              # Build and publish all platforms"
      exit 0
      ;;
  esac
  shift
done

VERSION="$(node -p "require('$MAIN_PKG').version")"
if [[ -z "$VERSION" ]]; then
  echo "ERROR: Failed to read version from $MAIN_PKG"
  exit 1
fi

CLI_CARGO_TOML="$ROOT_DIR/tools/lingxia-cli/Cargo.toml"
current_cargo_version="$(awk -F\" '/^version =/ {print $2; exit}' "$CLI_CARGO_TOML")"
if [[ -z "$current_cargo_version" ]]; then
  echo "ERROR: Failed to read version from $CLI_CARGO_TOML"
  exit 1
fi
if [[ "$current_cargo_version" != "$VERSION" ]]; then
  if [[ "$PUBLISH" -eq 1 ]]; then
    echo "ERROR: Rust CLI version ($current_cargo_version) does not match npm version ($VERSION)."
    echo "Please update tools/lingxia-cli/Cargo.toml to $VERSION before publishing."
    exit 1
  fi
  echo "Syncing Rust CLI version $current_cargo_version -> $VERSION"
  perl -0pi -e "s/^version\\s*=\\s*\\\"[^\\\"]+\\\"/version = \\\"$VERSION\\\"/m" "$CLI_CARGO_TOML"
fi

optional_mismatches="$(node -e "
const pkg = require('${MAIN_PKG}');
const v = pkg.version;
const opt = pkg.optionalDependencies || {};
const bad = Object.entries(opt).filter(([k,val]) => k.startsWith('@lingxia/cli-') && val !== v);
console.log(bad.map(([k,val]) => k + ':' + val).join(','));
")"
if [[ -n "$optional_mismatches" ]]; then
  if [[ "$PUBLISH" -eq 1 ]]; then
    echo "ERROR: optionalDependencies versions do not match npm version ($VERSION):"
    echo "  $optional_mismatches"
    echo "Please update tools/lingxia-cli/npm/package.json before publishing."
    exit 1
  fi
  echo "Syncing optionalDependencies to version $VERSION"
  node -e "
const fs = require('fs');
const pkgPath = '${MAIN_PKG}';
const pkg = JSON.parse(fs.readFileSync(pkgPath, 'utf-8'));
const v = pkg.version;
pkg.optionalDependencies = pkg.optionalDependencies || {};
for (const name of Object.keys(pkg.optionalDependencies)) {
  if (name.startsWith('@lingxia/cli-')) {
    pkg.optionalDependencies[name] = v;
  }
}
fs.writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + '\\n');
"
fi

# Function to build and package a single target
build_target() {
  local PLATFORM_NAME=$1
  local RUST_TARGET=$2
  local OS=$3
  local CPU=$4
  local BIN_EXT=$5

  echo "========================================"
  echo "Building $PLATFORM_NAME (version $VERSION)"
  echo "========================================"

  if [[ "$SKIP_BUILD" -eq 0 ]]; then
    echo "Building Rust CLI for $RUST_TARGET..."
    cd "$ROOT_DIR"
    cargo build -p lingxia-cli --release --target "$RUST_TARGET"
  fi

  BIN_SRC="$ROOT_DIR/target/$RUST_TARGET/release/lingxia$BIN_EXT"
  if [[ ! -f "$BIN_SRC" ]]; then
    echo "ERROR: Binary not found at $BIN_SRC"
    return 1
  fi

  PKG_DIR="$OUT_DIR/$PLATFORM_NAME"
  mkdir -p "$PKG_DIR/bin"

  cp "$BIN_SRC" "$PKG_DIR/bin/lingxia$BIN_EXT"
  chmod +x "$PKG_DIR/bin/lingxia$BIN_EXT"

  cat > "$PKG_DIR/package.json" <<EOF
{
  "name": "@lingxia/cli-$PLATFORM_NAME",
  "version": "$VERSION",
  "private": false,
  "os": ["$OS"],
  "cpu": ["$CPU"],
  "bin": {
    "lingxia": "bin/lingxia$BIN_EXT"
  },
  "files": ["bin/lingxia$BIN_EXT"],
  "license": "MIT"
}
EOF

  echo "✓ Generated platform package at $PKG_DIR"

  if [[ "$PUBLISH" -eq 1 ]]; then
    echo "Publishing @lingxia/cli-$PLATFORM_NAME..."
    cd "$PKG_DIR"
    npm publish --access public
    echo "✓ Published @lingxia/cli-$PLATFORM_NAME"
  fi
}

# Setup output directory
if [[ -z "$OUT_DIR" ]]; then
  if [[ "$TARGET" == "all" || "$PUBLISH" -eq 1 ]]; then
    # For multi-platform builds, keep output by default
    OUT_DIR="$START_DIR/dist"
    mkdir -p "$OUT_DIR"
    CLEAN=0
  else
    # For single platform, use temp dir
    OUT_DIR="$(mktemp -d /tmp/lingxia-cli-platform-XXXX)"
    CLEAN=1
  fi
else
  if [[ "$OUT_DIR" != /* ]]; then
    OUT_DIR="$START_DIR/$OUT_DIR"
  fi
  mkdir -p "$OUT_DIR"
fi

if [[ "$PUBLISH" -eq 1 ]]; then
  # Build all platforms for publishing
  echo "Building all platforms for publishing..."

  build_target "darwin-x64" "x86_64-apple-darwin" "darwin" "x64" ""
  build_target "darwin-arm64" "aarch64-apple-darwin" "darwin" "arm64" ""
  build_target "win32-x64" "x86_64-pc-windows-gnu" "win32" "x64" ".exe"
  build_target "win32-arm64" "aarch64-pc-windows-gnu" "win32" "arm64" ".exe"

  echo ""
  echo "========================================"
  echo "Publishing main JS package @lingxia/cli..."
  echo "========================================"
  cd "$ROOT_DIR/tools/lingxia-cli/npm"
  npm publish --access public

  echo ""
  echo "✅ All packages published successfully!"
elif [[ "$TARGET" == "all" ]]; then
  # Build all platforms without publishing
  echo "Building all platforms..."

  build_target "darwin-x64" "x86_64-apple-darwin" "darwin" "x64" ""
  build_target "darwin-arm64" "aarch64-apple-darwin" "darwin" "arm64" ""
  build_target "win32-x64" "x86_64-pc-windows-gnu" "win32" "x64" ".exe"
  build_target "win32-arm64" "aarch64-pc-windows-gnu" "win32" "arm64" ".exe"

  echo ""
  echo "✅ All platforms built successfully!"
  echo "   Output directory: $OUT_DIR"
elif [[ -n "$TARGET" ]]; then
  # Build specific target
  case "$TARGET" in
    darwin-x64)
      build_target "darwin-x64" "x86_64-apple-darwin" "darwin" "x64" ""
      ;;
    darwin-arm64)
      build_target "darwin-arm64" "aarch64-apple-darwin" "darwin" "arm64" ""
      ;;
    win32-x64)
      build_target "win32-x64" "x86_64-pc-windows-gnu" "win32" "x64" ".exe"
      ;;
    win32-arm64)
      build_target "win32-arm64" "aarch64-pc-windows-gnu" "win32" "arm64" ".exe"
      ;;
    *)
      echo "Error: Unknown target '$TARGET'"
      echo "Valid targets: darwin-x64, darwin-arm64, win32-x64, win32-arm64, all"
      exit 1
      ;;
  esac

  echo ""
  echo "✅ Build complete for $TARGET"
  echo "   Output directory: $OUT_DIR"
else
  # Build only current platform
  ARCH="$(uname -m)"
  OS_TYPE="$(uname -s)"

  case "$OS_TYPE" in
    Darwin)
      case "$ARCH" in
        x86_64) build_target "darwin-x64" "x86_64-apple-darwin" "darwin" "x64" "" ;;
        arm64) build_target "darwin-arm64" "aarch64-apple-darwin" "darwin" "arm64" "" ;;
        *) echo "Unsupported arch: $ARCH"; exit 1 ;;
      esac
      ;;
    MINGW*|MSYS*|CYGWIN*)
      case "$ARCH" in
        x86_64) build_target "win32-x64" "x86_64-pc-windows-gnu" "win32" "x64" ".exe" ;;
        aarch64) build_target "win32-arm64" "aarch64-pc-windows-gnu" "win32" "arm64" ".exe" ;;
        *) echo "Unsupported arch: $ARCH"; exit 1 ;;
      esac
      ;;
    *)
      echo "Unsupported OS: $OS_TYPE"
      exit 1
      ;;
  esac

  echo ""
  echo "✅ Build complete. Output directory: $OUT_DIR"
  echo "   Use --target all to build all platforms."
fi

if [[ "$CLEAN" -eq 1 ]]; then
  rm -rf "$OUT_DIR"
fi
