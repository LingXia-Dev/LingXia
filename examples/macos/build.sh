#!/bin/bash
set -e

# Function to handle errors
handle_error() {
    echo "Error: Build failed at line $1"
    exit 1
}

# Set error trap
trap 'handle_error $LINENO' ERR

# Get script directory and project paths
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
LINGXIA_ROOT="$SCRIPT_DIR/../.."
WORKSPACE_ROOT="$LINGXIA_ROOT" # Workspace root is the same as LingXia root

cd "$SCRIPT_DIR"

echo "Building Swift project for debugging..."

# Set the project root environment variable for Package.swift
export LINGXIA_PROJECT_ROOT="$(cd ../../ && pwd)"

if [ "$(uname -m)" = "arm64" ]; then
    # For Apple Silicon Macs
    swift build --arch arm64
else
    # For Intel Macs
    swift build
fi

# Check if build was successful
if [ $? -eq 0 ]; then
    echo "Build successful!"

    # Determine the correct binary path
    if [ "$(uname -m)" = "arm64" ]; then
        BINARY_PATH="./.build/arm64-apple-macosx/debug/LingXiaDemo"
    else
        BINARY_PATH="./.build/x86_64-apple-macosx/debug/LingXiaDemo"
    fi

    # Check if binary exists
    if [ -f "$BINARY_PATH" ]; then
        # Create app bundle
        echo "Creating app bundle..."
        if [ "$(uname -m)" = "arm64" ]; then
            BUILD_DIR="./.build/arm64-apple-macosx/debug"
        else
            BUILD_DIR="./.build/x86_64-apple-macosx/debug"
        fi

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

        # Copy other resources
        if [ -d "$BUILD_DIR/LingXiaDemo_LingXiaDemo.bundle/Resources" ]; then
            cp -r "$BUILD_DIR/LingXiaDemo_LingXiaDemo.bundle/Resources"/* "$APP_BUNDLE/Contents/Resources/"
        fi

        echo "App bundle created at $APP_BUNDLE"
        echo "Starting LingXiaDemo..."
        # Check for command line arguments and pass them to the app
        if [ $# -gt 0 ]; then
            "$APP_BUNDLE/Contents/MacOS/LingXiaDemo" "$@"
        else
            "$APP_BUNDLE/Contents/MacOS/LingXiaDemo"
        fi

        # Wait a moment for the app to start
        sleep 1

        # Check if the app started successfully (brief check)
        if pgrep -f "LingXiaDemo" > /dev/null; then
            echo "✅ LingXiaDemo started successfully"
            echo "ℹ️  App is running. Close the window to exit, or it will exit automatically when the last window closes."
        else
            # App might have exited quickly (user closed window) - check if it at least started
            echo "ℹ️  LingXiaDemo process not found - either failed to start or user closed it quickly"
            echo "ℹ️  This is normal if you closed the window immediately after startup"
        fi
    else
        echo "Error: Binary not found at $BINARY_PATH"
        exit 1
    fi
else
    echo "Error: Build failed"
    exit 1
fi
