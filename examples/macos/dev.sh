#!/bin/bash

# Dev build & run LingXia Example macOS App

set -euo pipefail

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
source "$SCRIPT_DIR/../scripts/common.sh"
init_common_vars
WORKSPACE_ROOT="$LINGXIA_ROOT"

# Parse command line arguments
for arg in "$@"; do
    if ! parse_common_arg "$arg"; then
        case "$arg" in
            --help|-h)
                show_help
                exit 0
                ;;
            *)
                echo "Unknown argument: $arg"
                echo "Use --help for usage information"
                exit 1
                ;;
        esac
    fi
done

# Define the resources directory for macOS
RESOURCES_DIR="$SCRIPT_DIR/Sources/Resources"

# Determine architecture
if [ "$(uname -m)" = "arm64" ]; then
    ARCH="arm64"
    RUST_TARGET="aarch64-apple-darwin"
    BUILD_DIR=".build/arm64-apple-macosx/debug"
else
    ARCH="x86_64"
    RUST_TARGET="x86_64-apple-darwin"
    BUILD_DIR=".build/x86_64-apple-macosx/debug"
fi

echo "[0/4] Preparing macOS SDK resources..."
SKIP_RUST=$SKIP_RUST bash "$LINGXIA_ROOT/lingxia-sdk/release.sh" \
    --platform apple \
    --apple-no-zip \
    --no-shasums \
    --out "$LINGXIA_ROOT/target/sdk-dev"

if [ "$SKIP_RUST" = false ]; then
    echo "[1/4] Building Rust library for macOS ($ARCH)..."
    cd "$WORKSPACE_ROOT"

    # Build lingxia-lib as staticlib for macOS
    if [ -n "$LXAPP_FEATURES" ]; then
        echo "  → Building lingxia-lib (staticlib) with features: $LXAPP_FEATURES"
        cargo rustc --crate-type=staticlib --target $RUST_TARGET --release -p lingxia-lib --features "$LXAPP_FEATURES"
    else
        echo "  → Building lingxia-lib (staticlib)..."
        cargo rustc --crate-type=staticlib --target $RUST_TARGET --release -p lingxia-lib
    fi

    echo "✅ Rust build complete"
    echo "   .a location: $WORKSPACE_ROOT/target/$RUST_TARGET/release/liblingxia.a"

    # SwiftPM may not detect updates to externally linked static libraries.
    # Clean Swift package artifacts to force relink against the freshly built liblingxia.a.
    echo "   ↻ Cleaning Swift package build artifacts to force relink..."
    cd "$SCRIPT_DIR"
    swift package clean
else
    echo "⏭️  Skipping Rust compilation (using existing libraries)"
fi

echo "[2/4] Preparing app resources..."
mkdir -p "$RESOURCES_DIR"
rm -rf "$RESOURCES_DIR/homelxapp" 2>/dev/null || true

generate_app_config "$RESOURCES_DIR"
build_and_copy_runtime "$RESOURCES_DIR" "es2020" "desktop"
build_and_copy_homelxapp "$RESOURCES_DIR"

echo "[3/4] Building Swift project..."
cd "$SCRIPT_DIR"

# Set the project root environment variable for Package.swift
export LINGXIA_PROJECT_ROOT="$LINGXIA_ROOT"

swift build --arch $ARCH

BINARY_PATH="$BUILD_DIR/LingXiaDemo"

if [ ! -f "$BINARY_PATH" ]; then
    echo "❌ Error: Binary not found at $BINARY_PATH"
    exit 1
fi

echo "[4/4] Creating app bundle and launching..."

APP_BUNDLE="$BUILD_DIR/LingXiaDemo.app"
rm -rf "$APP_BUNDLE"
mkdir -p "$APP_BUNDLE/Contents/MacOS"
mkdir -p "$APP_BUNDLE/Contents/Resources"

# Copy executable
cp "$BINARY_PATH" "$APP_BUNDLE/Contents/MacOS/"

# Copy Info.plist
cp "Info.plist" "$APP_BUNDLE/Contents/"

# Copy app icon if it exists
if [ -f "Sources/Resources/AppIcon.png" ]; then
    cp "Sources/Resources/AppIcon.png" "$APP_BUNDLE/Contents/Resources/"
fi

# Copy app bundle (homelxapp, app.json, etc.) - keep as bundle for detect_app_bundle
if [ -d "$BUILD_DIR/LingXiaDemo_LingXiaDemo.bundle" ]; then
    cp -r "$BUILD_DIR/LingXiaDemo_LingXiaDemo.bundle" "$APP_BUNDLE/Contents/Resources/"
fi

# Copy SDK bundle (icons, localization)
if [ -d "$BUILD_DIR/lingxia_lingxia.bundle" ]; then
    cp -r "$BUILD_DIR/lingxia_lingxia.bundle" "$APP_BUNDLE/Contents/Resources/"
fi

echo "✅ App bundle created at $APP_BUNDLE"
echo ""
echo "Starting LingXiaDemo..."
"$APP_BUNDLE/Contents/MacOS/LingXiaDemo" &
APP_PID=$!

# Wait a moment for the app to start
sleep 2

# Check if the app started successfully
if kill -0 $APP_PID 2>/dev/null; then
    echo "✅ LingXiaDemo started successfully (PID: $APP_PID)"
    echo "ℹ️  App is running. Close the window to exit."
else
    echo "ℹ️  LingXiaDemo process exited - this is normal if you closed the window"
fi
