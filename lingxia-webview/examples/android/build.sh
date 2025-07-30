#!/bin/bash

# Exit on error
set -e

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
PROJECT_ROOT="$SCRIPT_DIR/../.."
LINGXIA_ROOT="$PROJECT_ROOT/../" # LingXia project root directory
WORKSPACE_ROOT="$LINGXIA_ROOT" # Workspace root is the same as LingXia root

# Package name of the app
APP_PACKAGE="com.lingxia.example.miniapp"
MAIN_ACTIVITY="$APP_PACKAGE.MainActivity"

# Define the assets directory
ASSETS_DIR="$SCRIPT_DIR/app/src/main/assets"

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

# Build Rust library (unless skipped)
if [ "$SKIP_RUST" = false ]; then
    echo "Building Rust library..."

    # Set JAR output directory to Gradle build directory
    GRADLE_BUILD_DIR="$PROJECT_ROOT/android/lingxia/build/generated/lingxia-webview"
    mkdir -p "$GRADLE_BUILD_DIR"
    export LINGXIA_JAR_OUTPUT_DIR="$GRADLE_BUILD_DIR"

    # First build lingxia-webview to generate JAR
    echo "Building lingxia-webview to generate JAR..."
    cd "$WORKSPACE_ROOT"

    env \
    CMAKE_CONFIGURE_ARGS="-DCMAKE_TOOLCHAIN_FILE=$ANDROID_NDK_HOME/build/cmake/android.toolchain.cmake -DCMAKE_SYSTEM_PROCESSOR=aarch64"  \
    AR_aarch64_linux_android="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64/bin/llvm-ar" \
    CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64/bin/aarch64-linux-android33-clang" \
    CC_aarch64_linux_android="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64/bin/aarch64-linux-android33-clang" \
    CXX_aarch64_linux_android="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64/bin/aarch64-linux-android33-clang++" \
    cargo build --target aarch64-linux-android --release -p lingxia

    echo "Copying Rust library to jniLibs..."
    JNILIBS_DIR="$PROJECT_ROOT/android/lingxia/src/main/jniLibs/arm64-v8a"
    mkdir -p "$JNILIBS_DIR"
    cp "$WORKSPACE_ROOT/target/aarch64-linux-android/release/liblingxia.so" "$JNILIBS_DIR/"
else
    echo "⏭️  Skipping Rust compilation (using existing library)"
fi

# Create assets directory if it doesn't exist
mkdir -p "$ASSETS_DIR"

# Clean assets directory before copying new files
echo "Cleaning assets directory..."
rm -rf "$ASSETS_DIR"/*

echo "Copying lingxia-view files to assets..."
cp "$LINGXIA_ROOT/lingxia-view/404.html" "$ASSETS_DIR/"
cp "$LINGXIA_ROOT/lingxia-view/webview-bridge.js" "$ASSETS_DIR/"

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

echo "Building Android library..."
cd "$PROJECT_ROOT/android"
# ./gradlew :lingxia:clean
./gradlew :lingxia:assembleDebug

echo "Building and installing Android app..."
cd "$SCRIPT_DIR"
# ./gradlew clean
./gradlew assembleDebug

adb devices
adb install -r ./app/build/outputs/apk/debug/app-debug.apk

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
