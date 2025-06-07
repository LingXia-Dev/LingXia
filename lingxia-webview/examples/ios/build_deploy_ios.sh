#!/bin/bash

# Exit on error
set -e

# Get the absolute path of the script directory
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$SCRIPT_DIR/../.."
LINGXIA_ROOT="$PROJECT_ROOT/../" # LingXia project root directory
WORKSPACE_ROOT="$LINGXIA_ROOT" # Workspace root is the same as LingXia root

# Define the resources directory for iOS
RESOURCES_DIR="$SCRIPT_DIR/miniapp/Sources/miniapp/Resources"

echo "Building Rust library for iOS with Swift bridge headers..."
cd "$WORKSPACE_ROOT"

cargo rustc --crate-type=staticlib --release  --target aarch64-apple-ios -p lingxia

mkdir -p "$RESOURCES_DIR"

# Clean resources directory before copying new files
echo "Cleaning resources directory..."
rm -rf "$RESOURCES_DIR"/*

echo "Copying lingxia-view files to resources..."
cp "$LINGXIA_ROOT/lingxia-view/404.html" "$RESOURCES_DIR/"
cp "$LINGXIA_ROOT/lingxia-view/webview-bridge.js" "$RESOURCES_DIR/"

echo "Copying host app configuration..."
cp "$LINGXIA_ROOT/examples/demo/app.json" "$RESOURCES_DIR/"

echo "Building and copying demo MiniApp..."
# Build homeminiapp using LingXia MiniApp Builder
cd "$LINGXIA_ROOT/examples/demo/homeminiapp"
if [ -f "package.json" ] && [ -f "vite.config.js" ]; then
    echo "Building homeminiapp with Vite..."
    npm install --silent
    npm run build

    # Copy built MiniApp to resources with proper directory structure
    if [ -d "dist" ]; then
        echo "Copying built MiniApp to resources..."
        mkdir -p "$RESOURCES_DIR/homeminiapp"
        cp -R dist/* "$RESOURCES_DIR/homeminiapp/"
    else
        echo "Warning: dist directory not found, copying source files..."
        cp -R . "$RESOURCES_DIR/homeminiapp/"
    fi
else
    echo "No Vite config found, copying source files..."
    mkdir -p "$RESOURCES_DIR/homeminiapp"
    cp -R "$LINGXIA_ROOT/examples/demo/homeminiapp/"* "$RESOURCES_DIR/homeminiapp/"
fi

echo "Building and deploying iOS app..."
cd "$SCRIPT_DIR/miniapp"
xtool dev
