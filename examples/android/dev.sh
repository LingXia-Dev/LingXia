#!/bin/bash

# Dev build & run LingXia Example Android App
#
# Architecture:
#   SDK (lingxia-sdk/android/):
#     - Kotlin AAR: LxApp, NativeApi, etc.
#     - Rust .a: liblingxia_core.a (framework core)
#
#   App (examples/android/):
#     - lingxia-lib crate: native library + user extensions
#     - Output: liblingxia.so (final linked library)

set -euo pipefail

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
source "$SCRIPT_DIR/../scripts/common.sh"
init_common_vars
WORKSPACE_ROOT="$LINGXIA_ROOT"
LINGXIA_SDK_ANDROID="$LINGXIA_ROOT/lingxia-sdk/android"

# Android-specific options
BUILD_ARMV7=false
USE_ES5_RUNTIME=false

# Parse command line arguments
for arg in "$@"; do
    if ! parse_common_arg "$arg"; then
        case $arg in
            --arm32|arm32|with-arm32|with-armv7)
                BUILD_ARMV7=true
                echo "✅ Enabling 32-bit (armeabi-v7a) build"
                USE_ES5_RUNTIME=true
                echo "✅ Enabling ES5 web runtime (recommended for older Android WebView)"
                ;;
            --es5|es5|legacy|with-es5)
                USE_ES5_RUNTIME=true
                echo "✅ Enabling ES5 web runtime"
                ;;
            --es2020|es2020|modern)
                USE_ES5_RUNTIME=false
                echo "✅ Using modern web runtime"
                ;;
            --help|-h)
                show_help "  --arm32                 Enable 32-bit build
  --es5                   Enable ES5 web runtime"
                exit 0
                ;;
            --framework)
                # Handle --framework vue (next arg) - skip
                ;;
            *)
                echo "Unknown argument: $arg"
                echo "Use --help for usage information"
                exit 1
                ;;
        esac
    fi
done
BASE_SDK_VERSION="$(awk '
  /^\[workspace\.package\]/ {in_section=1; next}
  /^\[/ {in_section=0}
  in_section && $1 == "version" {
    gsub(/"/, "", $3);
    print $3;
    exit
  }' "$LINGXIA_ROOT/Cargo.toml")"
if [ -z "$BASE_SDK_VERSION" ]; then
    echo "Failed to read workspace version from Cargo.toml" >&2
    exit 1
fi
LINGXIA_SDK_VERSION="$BASE_SDK_VERSION"
ANDROID_ES5_FLAG=""
if $USE_ES5_RUNTIME; then
    ANDROID_ES5_FLAG="--android-es5"
    LINGXIA_SDK_VERSION="${BASE_SDK_VERSION}-es5"
fi

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

LOCAL_MAVEN_DIR="$LINGXIA_ROOT/target/maven"

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

echo "[0/4] Preparing Android SDK (resources + local Maven)..."
bash "$LINGXIA_ROOT/lingxia-sdk/release.sh" \
  --platform android \
  $ANDROID_ES5_FLAG \
  --android-maven-dir "$LOCAL_MAVEN_DIR" \
  --android-no-zip \
  --no-shasums

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

echo "[2/4] Local Maven ready: $LOCAL_MAVEN_DIR (version: $LINGXIA_SDK_VERSION)"

# Create assets directory if it doesn't exist
mkdir -p "$ASSETS_DIR"

# Clean assets directory before copying new files
echo "Cleaning assets directory..."
rm -rf "$ASSETS_DIR"/*

generate_app_config "$ASSETS_DIR"
build_and_copy_homelxapp "$ASSETS_DIR"

echo "[4/4] Building and installing Android example app (release)..."
cd "$SCRIPT_DIR"
# ./gradlew clean
./gradlew assembleRelease -PLINGXIA_SDK_VERSION="$LINGXIA_SDK_VERSION"

echo "Installing APK on all connected devices..."
for device in $(adb devices | awk 'NR>1 && NF>0 {print $1}'); do
    if [ "$CLEAN_INSTALL" = true ]; then
        echo "Uninstalling from $device..."
        adb -s "$device" uninstall "$APP_PACKAGE" 2>/dev/null || true
    fi
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
