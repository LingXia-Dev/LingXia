#!/bin/bash

# Exit on error
set -e

# Parse command line arguments
SKIP_RUST=false
for arg in "$@"; do
    case $arg in
        --skip-rust)
        SKIP_RUST=true
        shift
        ;;
        *)
        # Keep other arguments for passing to build.sh
        ;;
    esac
done

# Get the absolute path of the script directory
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$SCRIPT_DIR/../.."  # lingxia-webview directory
LINGXIA_ROOT="$SCRIPT_DIR/../../../" # LingXia project root directory
WORKSPACE_ROOT="$LINGXIA_ROOT" # Workspace root is the same as LingXia root

# Define the resources directory for macOS
RESOURCES_DIR="$SCRIPT_DIR/Sources/Resources"

if [ "$SKIP_RUST" = false ]; then
    echo "Building Rust library for macOS with Swift bridge headers..."
    cd "$WORKSPACE_ROOT"

    # Build for macOS (static library for linking)
    if [ "$(uname -m)" = "arm64" ]; then
        echo "Building for Apple Silicon (arm64)..."
        cargo rustc --crate-type=staticlib --release --target aarch64-apple-darwin -p lingxia --manifest-path lingxia-webview/Cargo.toml
    else
        echo "Building for Intel (x86_64)..."
        cargo rustc --crate-type=staticlib --release --target x86_64-apple-darwin -p lingxia --manifest-path lingxia-webview/Cargo.toml
    fi
else
    echo "Skipping Rust compilation (--skip-rust flag provided)"
fi

mkdir -p "$RESOURCES_DIR"

echo "Copying lingxia-view files to resources..."
cp "$LINGXIA_ROOT/lingxia-view/404.html" "$RESOURCES_DIR/"
cp "$LINGXIA_ROOT/lingxia-view/webview-bridge.js" "$RESOURCES_DIR/"

echo "Copying host app configuration..."
cp "$LINGXIA_ROOT/examples/demo/app.json" "$RESOURCES_DIR/"

mkdir -p "$RESOURCES_DIR/homelxapp"
rm -rf "$RESOURCES_DIR/homelxapp"/*
cp -r "$LINGXIA_ROOT/examples/demo/homelxapp/dist"/* "$RESOURCES_DIR/homelxapp/"

echo "Building and running macOS app..."
cd "$SCRIPT_DIR"

./build.sh "$@"
