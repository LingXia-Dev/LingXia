#!/bin/bash

# Build and run LingXia Example Android App
#
# Architecture:
#   SDK (lingxia-sdk/android/):
#     - Kotlin AAR: LxApp, NativeApi, etc.
#     - Rust .a: liblingxia_core.a (framework core)
#
#   App (examples/android/):
#     - lingxia-lib crate: native library + user extensions
#     - Output: liblingxia.so (final linked library)
#
# Usage: ./build.sh [skip-rust]

set -euo pipefail

# Parse command line arguments
SKIP_RUST=false
BUILD_ARMV7=false
for arg in "$@"; do
    case $arg in
        skip-rust)
            SKIP_RUST=true
            echo "🚀 Skipping Rust compilation"
            ;;
        arm32|with-arm32|with-armv7)
            BUILD_ARMV7=true
            echo "✅ Enabling 32-bit (armeabi-v7a) build"
            ;;
        *)
            echo "Unknown argument: $arg"
            echo "Usage: $0 [skip-rust] [arm32]"
            exit 1
            ;;
    esac
done

# Get the absolute path of the script directory
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
LINGXIA_ROOT="$SCRIPT_DIR/../.."
WORKSPACE_ROOT="$LINGXIA_ROOT"
LINGXIA_SDK_ANDROID="$LINGXIA_ROOT/lingxia-sdk/android"
LXAPP_FEATURES="${LXAPP_FEATURES:-}" # set via env var, e.g. LXAPP_FEATURES=cloud ./build.sh

# Package name of the app
APP_PACKAGE="com.lingxia.example.lxapp"
MAIN_ACTIVITY="$APP_PACKAGE.MainActivity"

# Define the assets directory
ASSETS_DIR="$SCRIPT_DIR/app/src/main/assets"

# App's jniLibs directory (NOT SDK's!)
APP_JNILIBS_DIR_ARM64="$SCRIPT_DIR/app/src/main/jniLibs/arm64-v8a"
APP_JNILIBS_DIR_ARMV7="$SCRIPT_DIR/app/src/main/jniLibs/armeabi-v7a"

# Default values for variables used later
LOGCAT_PID=""

# Function to cleanup and exit
cleanup() {
    echo "Cleaning up..."
    if [ ! -z "$LOGCAT_PID" ]; then
        kill $LOGCAT_PID 2>/dev/null || true
    fi
}

trap cleanup EXIT

GRADLE_BUILD_DIR="$LINGXIA_SDK_ANDROID/lingxia/build/generated/lingxia-webview"
mkdir -p "$GRADLE_BUILD_DIR"
export LINGXIA_JAR_OUTPUT_DIR="$GRADLE_BUILD_DIR"

build_rust_android() {
    local target="$1"
    local cmake_proc="$2"
    local cc_bin="$3"
    local cxx_bin="$4"

    # Configure CMake for the target CPU
    export CMAKE_CONFIGURE_ARGS="-DCMAKE_TOOLCHAIN_FILE=$ANDROID_NDK_HOME/build/cmake/android.toolchain.cmake -DCMAKE_SYSTEM_PROCESSOR=$cmake_proc"

    # Avoid macOS SDK pollution when cross-compiling
    unset SDKROOT CMAKE_OSX_SYSROOT CMAKE_OSX_ARCHITECTURES MACOSX_DEPLOYMENT_TARGET
    # CRITICAL: Do NOT set CMAKE_TOOLCHAIN_FILE - let cmake-rs handle Android configuration
    unset CMAKE_TOOLCHAIN_FILE
    # Set ANDROID_NDK_ROOT for aws-lc-sys to find the NDK
    export ANDROID_NDK_ROOT="$ANDROID_NDK_HOME"
    export ANDROID_NDK="$ANDROID_NDK_HOME"

    case "$target" in
        aarch64-linux-android)
            export AR_aarch64_linux_android="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64/bin/llvm-ar"
            export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64/bin/$cc_bin"
            export CC_aarch64_linux_android="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64/bin/$cc_bin"
            export CXX_aarch64_linux_android="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64/bin/$cxx_bin"
            ;;
        armv7-linux-androideabi)
            export AR_armv7_linux_androideabi="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64/bin/llvm-ar"
            export CARGO_TARGET_ARMV7_LINUX_ANDROIDEABI_LINKER="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64/bin/$cc_bin"
            export CC_armv7_linux_androideabi="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64/bin/$cc_bin"
            export CXX_armv7_linux_androideabi="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64/bin/$cxx_bin"
            ;;
        *)
            echo "Unsupported target: $target" >&2
            exit 1
            ;;
    esac

    # Clean CMake cache for aws-lc-sys to avoid cross-compilation issues
    # (CMake caches host toolchain settings that conflict with Android NDK)
    rm -rf "$WORKSPACE_ROOT/target/$target/release/build/aws-lc-sys-"*/out/build/CMakeCache.txt
    rm -rf "$WORKSPACE_ROOT/target/$target/release/build/aws-lc-sys-"*/out/build/CMakeFiles

    # Build lingxia-lib (native library + user extensions)
    if [ -n "$LXAPP_FEATURES" ]; then
        echo "  → Building lingxia-lib ($target) with features: $LXAPP_FEATURES"
        cargo build --target $target --release -p lingxia-lib --features "$LXAPP_FEATURES"
    else
        echo "  → Building lingxia-lib ($target)..."
        cargo build --target $target --release -p lingxia-lib
    fi
}

# Generate i18n resources for Android
echo "Generating i18n resources for Android..."
cargo run -p lingxia-gen -- i18n \
  --input "$LINGXIA_ROOT/i18n" \
  --android-out "$LINGXIA_SDK_ANDROID/lingxia/src/main/res"

# Generate asset manifests for Android
echo "Generating asset manifests for Android..."
cargo run -p lingxia-gen -- assets \
  --input "$LINGXIA_ROOT/lingxia-sdk/resources/assets" \
  --android-out "$LINGXIA_SDK_ANDROID/lingxia/src/main/assets"

if [ "$SKIP_RUST" = false ]; then
    echo "[1/4] Building Rust libraries..."
    cd "$WORKSPACE_ROOT"
    # Build lingxia-lib for 64-bit Android
    build_rust_android "aarch64-linux-android" "aarch64" "aarch64-linux-android33-clang" "aarch64-linux-android33-clang++"

    # Optionally build 32-bit Android when requested
    if [ "$BUILD_ARMV7" = true ]; then
        build_rust_android "armv7-linux-androideabi" "armv7-a" "armv7a-linux-androideabi33-clang" "armv7a-linux-androideabi33-clang++"
    fi

    # Copy .so to App's jniLibs
    echo "  → Copying liblingxia_lib.so to App jniLibs (arm64-v8a)..."
    mkdir -p "$APP_JNILIBS_DIR_ARM64"
    cp "$WORKSPACE_ROOT/target/aarch64-linux-android/release/liblingxia_lib.so" "$APP_JNILIBS_DIR_ARM64/liblingxia.so"

    if [ "$BUILD_ARMV7" = true ]; then
        echo "  → Copying liblingxia_lib.so to App jniLibs (armeabi-v7a)..."
        mkdir -p "$APP_JNILIBS_DIR_ARMV7"
        cp "$WORKSPACE_ROOT/target/armv7-linux-androideabi/release/liblingxia_lib.so" "$APP_JNILIBS_DIR_ARMV7/liblingxia.so"
    fi

    echo "✅ Rust build complete"
    if [ "$BUILD_ARMV7" = true ]; then
        echo "   .so locations:"
        echo "     - $APP_JNILIBS_DIR_ARM64/liblingxia.so"
        echo "     - $APP_JNILIBS_DIR_ARMV7/liblingxia.so"
    else
        echo "   .so location: $APP_JNILIBS_DIR_ARM64/liblingxia.so"
    fi
else
    echo "⏭️  Skipping Rust compilation (using existing libraries)"
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

echo "Installing APK on all connected devices..."
for device in $(adb devices | awk 'NR>1 && NF>0 {print $1}'); do
    echo "Installing on $device..."
    adb -s "$device" install -r ./app/build/outputs/apk/release/app-release.apk
done

echo "Starting logcat capture..."
# Clear existing logs
adb logcat -c

echo "Launching app..."
adb shell am start -n "$APP_PACKAGE/$MAIN_ACTIVITY"

# Show logs directly in terminal (include HelloExt for extension logs)
echo "Showing logs (will auto-stop after 1 minute)..."
adb logcat -v time Rust:I HelloExt:I LingXia:I WebView:D *:S &
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
