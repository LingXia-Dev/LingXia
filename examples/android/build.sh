#!/bin/bash

# Exit on error
set -euo pipefail

# Parse command line arguments
SKIP_RUST=false
for arg in "$@"; do
    case $arg in
        skip-rust)
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
WORKSPACE_ROOT="$LINGXIA_ROOT" # Workspace root is the same as LingXia root
LINGXIA_SDK_ANDROID="$LINGXIA_ROOT/lingxia-sdk/android" # LingXia SDK Android directory

# Package name of the app
APP_PACKAGE="com.lingxia.example.lxapp"
MAIN_ACTIVITY="$APP_PACKAGE.MainActivity"

# Define the assets directory
ASSETS_DIR="$SCRIPT_DIR/app/src/main/assets"

# Default values for variables used later
LOGCAT_PID=""

# Function to cleanup and exit
cleanup() {
    echo "Cleaning up..."
    # Kill logcat process if it exists
    if [ ! -z "$LOGCAT_PID" ]; then
        kill $LOGCAT_PID 2>/dev/null || true
    fi
}

# Set trap for cleanup
trap cleanup EXIT

GRADLE_BUILD_DIR="$LINGXIA_SDK_ANDROID/lingxia/build/generated/lingxia-webview"
mkdir -p "$GRADLE_BUILD_DIR"
export LINGXIA_JAR_OUTPUT_DIR="$GRADLE_BUILD_DIR"

# 1) Build Rust lib (liblingxia.so) or skip
if [ "$SKIP_RUST" = false ]; then
    echo "[1/4] Building Rust library (liblingxia) ..."
    cd "$WORKSPACE_ROOT"
    env \
    CMAKE_CONFIGURE_ARGS="-DCMAKE_TOOLCHAIN_FILE=$ANDROID_NDK_HOME/build/cmake/android.toolchain.cmake -DCMAKE_SYSTEM_PROCESSOR=aarch64"  \
    AR_aarch64_linux_android="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64/bin/llvm-ar" \
    CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64/bin/aarch64-linux-android33-clang" \
    CC_aarch64_linux_android="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64/bin/aarch64-linux-android33-clang" \
    CXX_aarch64_linux_android="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64/bin/aarch64-linux-android33-clang++" \
    cargo build --target aarch64-linux-android --release -p lingxia-lib

    echo "Copying liblingxia.so to SDK jniLibs..."
    JNILIBS_DIR="$LINGXIA_SDK_ANDROID/lingxia/src/main/jniLibs/arm64-v8a"
    mkdir -p "$JNILIBS_DIR"
    cp "$WORKSPACE_ROOT/target/aarch64-linux-android/release/liblingxia.so" "$JNILIBS_DIR/"
else
    echo "⏭️  Skipping Rust compilation (using existing library)"
fi

# Ensure lingxia-webview.jar exists; if missing (skip-rust path), build via Makefile
LINGXIA_WEBVIEW_JAR="$GRADLE_BUILD_DIR/lingxia-webview.jar"
if [ ! -f "$LINGXIA_WEBVIEW_JAR" ]; then
    echo "Building lingxia-webview.jar via Makefile (no Rust)..."
    JAVA_SRC_DIR="$WORKSPACE_ROOT/lingxia-webview/src/android/java"
    if [ ! -d "$JAVA_SRC_DIR" ]; then
        echo "❌ Java source dir not found: $JAVA_SRC_DIR"; exit 1
    fi
    (cd "$JAVA_SRC_DIR" && TARGET_DIR="$GRADLE_BUILD_DIR" make)
    if [ ! -f "$LINGXIA_WEBVIEW_JAR" ]; then
        echo "❌ Failed to create $LINGXIA_WEBVIEW_JAR"; exit 1
    fi
fi

echo "Copying lingxia-view files into SDK AAR assets..."
SDK_ASSETS_DIR="$LINGXIA_SDK_ANDROID/lingxia/src/main/assets"
mkdir -p "$SDK_ASSETS_DIR"
cp "$LINGXIA_ROOT/lingxia-view/404.html" "$SDK_ASSETS_DIR/"
cp "$LINGXIA_ROOT/lingxia-view/webview-bridge.js" "$SDK_ASSETS_DIR/"

echo "[2/4] Publishing SDK AAR (release) to local Maven ..."
cd "$LINGXIA_SDK_ANDROID"
LOCAL_MAVEN_DIR="$LINGXIA_ROOT/target/maven"
./gradlew :lingxia:publish -PLOCAL_MAVEN_REPO_DIR="$LOCAL_MAVEN_DIR"
if [ ! -f "$LOCAL_MAVEN_DIR/com/lingxia/lingxia/0.0.1/lingxia-0.0.1.aar" ]; then
    echo "❌ Failed to publish SDK AAR to $LOCAL_MAVEN_DIR"; exit 1
fi

# Create assets directory if it doesn't exist
mkdir -p "$ASSETS_DIR"

# Clean assets directory before copying new files
echo "Cleaning assets directory..."
rm -rf "$ASSETS_DIR"/*

echo "Copying host app configuration..."
cp "$LINGXIA_ROOT/examples/demo/app.json" "$ASSETS_DIR/"

echo "Building and copying demo LxApp..."
# Build homelxapp using LingXia Builder
cd "$LINGXIA_ROOT/examples/demo/homelxapp"
# Copy built LxApp to assets with proper directory structure
if [ -d "dist" ]; then
    echo "Copying built LxApp to assets..."
    mkdir -p "$ASSETS_DIR/homelxapp"
    cp -R dist/* "$ASSETS_DIR/homelxapp/"
    echo "✅ Successfully copied dist contents to assets/homelxapp"
    echo "📁 Contents copied:"
    ls -la "$ASSETS_DIR/homelxapp"
else
    echo "❌ Error: dist directory not found"
    echo "📁 Current directory contents:"
    ls -la .
    exit 1
fi

echo "[4/4] Building and installing Android example app (release)..."
cd "$SCRIPT_DIR"
# ./gradlew clean
./gradlew assembleRelease

adb devices
adb install -r ./app/build/outputs/apk/release/app-release.apk

echo "Starting logcat capture..."
# Clear existing logs
adb logcat -c

echo "Launching app..."
adb shell am start -n "$APP_PACKAGE/$MAIN_ACTIVITY"

# Show logs directly in terminal
echo "Showing logs (will auto-stop after 1 minute)..."
adb logcat -v time Rust:I WebView:D *:S &
LOGCAT_PID=$!

# Wait for 1 minute then kill logcat and exit
(
    sleep 60
    echo "Stopping logcat after 1 minute timeout..."
    kill $LOGCAT_PID 2>/dev/null
    exit 0
) &

# Wait for either the logcat process or the timeout
wait $LOGCAT_PID
