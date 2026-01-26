#!/bin/bash

# Dev build & deploy LingXia Example iOS App
#
# Usage: ./dev.sh [skip-rust]

set -euo pipefail

# Parse command line arguments
SKIP_RUST=false
for arg in "$@"; do
    case $arg in
        skip-rust|--skip-rust)
            SKIP_RUST=true
            echo "🚀 Skipping Rust compilation"
            ;;
        *)
            echo "Unknown argument: $arg"
            echo "Usage: $0 [skip-rust]"
            exit 1
            ;;
    esac
done

# Get the absolute path of the script directory
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
LINGXIA_ROOT="$SCRIPT_DIR/../.."
WORKSPACE_ROOT="$LINGXIA_ROOT"
LXAPP_FEATURES="${LXAPP_FEATURES:-}" # set via env var, e.g. LXAPP_FEATURES=cloud ./dev.sh

# Define the resources directory for iOS
RESOURCES_DIR="$SCRIPT_DIR/lxapp/Sources/lxapp/Resources"
echo "RESOURCES_DIR: $RESOURCES_DIR"

# Generate Swift bridge bindings before staging the Apple SDK into target/spm/lingxia.
# This creates/updates: lingxia-sdk/apple/Sources/generated/...
if [ "$SKIP_RUST" = false ]; then
    echo "[0/4] Generating Swift bridge bindings..."
    cd "$WORKSPACE_ROOT"

    TARGET="aarch64-apple-ios"
    LINGXIA_GENERATE_BRIDGE=1 cargo build -p lingxia --target $TARGET --release 2>&1 | grep -E "Generated|warning:" | head -5 || true
fi

echo "[1/4] Preparing iOS SDK resources..."
# For dev: generate Resources/icons/runtime assets via the unified SDK script.
bash "$LINGXIA_ROOT/lingxia-sdk/release.sh" \
  --platforms ios \
  --version dev \
  --ios-no-zip \
  --no-shasums \
  --out "$LINGXIA_ROOT/target/sdk-dev"

# Build Rust library for iOS unless skip-rust flag is set
if [ "$SKIP_RUST" = false ]; then
    echo "[2/4] Building Rust libraries..."
    cd "$WORKSPACE_ROOT"

    # Build lingxia-lib as staticlib for iOS (native library + user extensions)
    # Note: iOS requires staticlib (.a), not cdylib (.dylib)
    if [ -n "$LXAPP_FEATURES" ]; then
        echo "  → Building lingxia-lib (staticlib) with features: $LXAPP_FEATURES"
        cargo rustc --crate-type=staticlib --target $TARGET --release -p lingxia-lib --features "$LXAPP_FEATURES"
    else
        echo "  → Building lingxia-lib (staticlib)..."
        cargo rustc --crate-type=staticlib --target $TARGET --release -p lingxia-lib
    fi

    # Copy to expected name (liblingxia.a) for Xcode project compatibility
    cp "$WORKSPACE_ROOT/target/$TARGET/release/liblingxia_lib.a" "$WORKSPACE_ROOT/target/$TARGET/release/liblingxia.a"

    echo "✅ Rust build complete"
    echo "   .a location: $WORKSPACE_ROOT/target/$TARGET/release/liblingxia.a"
else
    echo "⏭️  Skipping Rust compilation (using existing libraries)"
fi

mkdir -p "$RESOURCES_DIR"

# Clean resources directory before copying new files
echo "Cleaning resources directory..."
rm -rf "$RESOURCES_DIR"/*

echo "Copying host app configuration..."
cp "$LINGXIA_ROOT/examples/demo/app.json" "$RESOURCES_DIR/"

echo "Building and copying demo LxApp..."
cd "$LINGXIA_ROOT/examples/demo/homelxapp"
if [ -f "package.json" ] ; then
    # Copy built LxApp to resources with proper directory structure
    if [ -d "dist" ]; then
        echo "Copying built LxApp to resources..."
        mkdir -p "$RESOURCES_DIR/homelxapp"
        cp -R dist/* "$RESOURCES_DIR/homelxapp/"
    else
        echo "Error: dist directory not found, copying source files..."
        exit 1
    fi
else
    echo "Error: package.json not found"
    exit 1
fi

echo "Building and deploying iOS app..."
cd "$SCRIPT_DIR/lxapp"
env LINGXIA_PROJECT_ROOT=$LINGXIA_ROOT xtool dev
