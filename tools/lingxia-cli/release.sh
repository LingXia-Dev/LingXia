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
    *) return 1 ;;
  esac
}

ALL_TARGETS="darwin-x64 darwin-arm64 win32-x64"

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
        *) echo ""; return 1 ;;
      esac ;;
    *) echo ""; return 1 ;;
  esac
}

# Show help if no arguments
if [[ $# -eq 0 ]]; then
  set -- -h
fi

# Parse arguments
PUBLISH=0
OUT_DIR=""
SKIP_BUILD=0
TARGET=""
BUMP_VERSION=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --publish) PUBLISH=1 ;;
    --out) OUT_DIR="$2"; shift ;;
    --skip-build) SKIP_BUILD=1 ;;
    --target) TARGET="$2"; shift ;;
    --bump) BUMP_VERSION="$2"; shift ;;
    -h|--help)
      cat <<EOF
Usage: release.sh [OPTIONS]

Options:
  --bump <version>     Bump all version files to specified version (e.g., 0.0.8)
                       Updates: package.json, Cargo.toml, optionalDependencies, package-lock.json
  --target <platform>  Build specific platform(s): darwin-x64, darwin-arm64, win32-x64, all
  --publish            Publish platform package(s) + main @lingxia/cli (requires all platforms)
  --out <dir>          Output directory (default: ./dist)
  --skip-build         Skip cargo build, use existing binaries

Examples:
  ./release.sh --bump 0.0.8                   # Bump version only
  ./release.sh --target darwin-x64            # Build Intel Mac
  ./release.sh --bump 0.0.8 --publish         # Bump + build all + publish
  ./release.sh --target all --publish         # Full release (all platforms)
EOF
      exit 0 ;;
    *) echo "Unknown option: $1"; exit 1 ;;
  esac
  shift
done

# Publishing wrapper package must include all optional platform packages.
if [[ "$PUBLISH" -eq 1 ]]; then
  if [[ -z "$TARGET" ]]; then
    TARGET="all"
  elif [[ "$TARGET" != "all" ]]; then
    echo "ERROR: --publish requires --target all (or omit --target)."
    echo "       Wrapper @lingxia/cli depends on all platform optionalDependencies."
    exit 1
  fi
fi

# Handle --bump: update all version files
if [[ -n "$BUMP_VERSION" ]]; then
  echo "Bumping version to $BUMP_VERSION..."
  
  # Validate version format
  if ! [[ "$BUMP_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "ERROR: Invalid version format. Use semver (e.g., 0.0.8)"
    exit 1
  fi
  
  # 1. Update package.json version
  node -e "
    const fs = require('fs');
    const pkg = JSON.parse(fs.readFileSync('$MAIN_PKG'));
    pkg.version = '$BUMP_VERSION';
    // Also update optionalDependencies
    for (const k of Object.keys(pkg.optionalDependencies || {})) {
      if (k.startsWith('@lingxia/cli-')) pkg.optionalDependencies[k] = '$BUMP_VERSION';
    }
    fs.writeFileSync('$MAIN_PKG', JSON.stringify(pkg, null, 2) + '\n');
  "
  echo "  ✓ Updated npm/package.json"
  
  # 2. Update Cargo.toml version
  sed -i.bak "s/^version = \"[^\"]*\"/version = \"$BUMP_VERSION\"/" "$CLI_CARGO_TOML" && rm -f "$CLI_CARGO_TOML.bak"
  echo "  ✓ Updated Cargo.toml"
  
  # 3. Update package-lock.json
  (cd "$ROOT_DIR/tools/lingxia-cli/npm" && npm install --package-lock-only --ignore-scripts 2>/dev/null)
  echo "  ✓ Updated npm/package-lock.json"
  
  echo ""
  echo "✅ Version bumped to $BUMP_VERSION"
  echo "   Files updated:"
  echo "   - tools/lingxia-cli/npm/package.json"
  echo "   - tools/lingxia-cli/npm/package-lock.json"
  echo "   - tools/lingxia-cli/Cargo.toml"
  
  # If no target specified, exit after bump
  if [[ -z "$TARGET" && "$PUBLISH" -eq 0 ]]; then
    exit 0
  fi
fi

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

# Sync package-lock.json version
lock_version="$(node -p "require('$ROOT_DIR/tools/lingxia-cli/npm/package-lock.json').version" 2>/dev/null || echo "")"
if [[ -n "$lock_version" && "$lock_version" != "$VERSION" ]]; then
  echo "Syncing package-lock.json version: $lock_version -> $VERSION"
  (cd "$ROOT_DIR/tools/lingxia-cli/npm" && npm install --package-lock-only --ignore-scripts)
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
if [[ -z "$TARGET" ]]; then
  # No target specified, nothing to build (--bump only mode is ok)
  if [[ -z "$BUMP_VERSION" ]]; then
    echo "ERROR: No action specified. Use --bump, --target, or --help"
    exit 1
  fi
  targets=()
elif [[ "$TARGET" == "all" ]]; then
  targets=($ALL_TARGETS)
else
  get_target_info "$TARGET" >/dev/null || { echo "Unknown target: $TARGET"; exit 1; }
  targets=("$TARGET")
fi

# Build all targets
for t in "${targets[@]}"; do
  build_target "$t"
done

wait_for_package() {
  local package_name="$1"
  local version="$2"
  local max_attempts="${3:-12}"
  local sleep_secs="${4:-5}"
  local attempt

  for attempt in $(seq 1 "$max_attempts"); do
    if npm view "${package_name}@${version}" version >/dev/null 2>&1; then
      return 0
    fi
    echo "Waiting for npm package ${package_name}@${version} (attempt ${attempt}/${max_attempts})..."
    sleep "$sleep_secs"
  done
  return 1
}

ensure_optional_deps_available() {
  local missing=0
  local deps
  deps="$(node -e "
    const pkg = require('$MAIN_PKG');
    const entries = Object.entries(pkg.optionalDependencies || {});
    for (const [name, version] of entries) {
      if (name.startsWith('@lingxia/cli-')) console.log(name + ' ' + version);
    }
  ")"

  if [[ -z "$deps" ]]; then
    echo "ERROR: No @lingxia/cli-* optionalDependencies found in $MAIN_PKG"
    return 1
  fi

  while read -r dep_name dep_version; do
    [[ -z "$dep_name" ]] && continue
    if wait_for_package "$dep_name" "$dep_version"; then
      echo "✓ Found ${dep_name}@${dep_version}"
    else
      echo "ERROR: Missing ${dep_name}@${dep_version} on npm."
      missing=1
    fi
  done <<< "$deps"

  if [[ "$missing" -ne 0 ]]; then
    echo "Refusing to publish @lingxia/cli@$VERSION because required platform packages are not available."
    return 1
  fi
}

# Publish main package
if [[ "$PUBLISH" -eq 1 ]]; then
  ensure_optional_deps_available

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
