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

echo "Building and copying demo LxApp..."
cd "$LINGXIA_ROOT/examples/demo/homelxapp"
if [ -f "package.json" ] && [ -f "vite.config.js" ]; then
    echo "Building homelxapp with Vite..."
    npm install --silent
    npm run build

    # Copy built LxApp to resources with proper directory structure
    if [ -d "dist" ]; then
        echo "Copying built LxApp to resources..."
        mkdir -p "$RESOURCES_DIR/homelxapp"
        cp -R dist/* "$RESOURCES_DIR/homelxapp/"
    else
        echo "Warning: dist directory not found, copying source files..."
        cp -R . "$RESOURCES_DIR/homelxapp/"
    fi
else
    echo "No Vite config found, copying source files..."
    mkdir -p "$RESOURCES_DIR/homelxapp"
    cp -R "$LINGXIA_ROOT/examples/demo/homelxapp/"* "$RESOURCES_DIR/homelxapp/"
fi

echo "Building and deploying iOS app..."
cd "$SCRIPT_DIR/miniapp"
xtool dev
