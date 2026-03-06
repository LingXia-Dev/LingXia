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
BUILD_ARM32=false
MIN_SDK=29

# Parse command line arguments
for arg in "$@"; do
    if ! parse_common_arg "$arg"; then
        case $arg in
            --arm32)
                BUILD_ARM32=true
                # Keep SDK defaults high, but allow app minSdk downgrade for arm32 dev.
                MIN_SDK=21
                ;;
            --help|-h)
                show_help "  --arm32       Build 32-bit (armeabi-v7a)"
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

# Mobile builds default to ring unless TLS backend is explicitly chosen.
ensure_tls_feature_default "tls-ring"

require_dir_env() {
    local var_name="$1"
    local example="$2"
    local value="${!var_name:-}"
    if [ -z "$value" ]; then
        echo "❌ $var_name is not set" >&2
        echo "   Example: export $var_name=$example" >&2
        exit 1
    fi
    if [ ! -d "$value" ]; then
        echo "❌ $var_name does not exist: $value" >&2
        exit 1
    fi
}

require_dir_env "ANDROID_SDK_ROOT" "\$HOME/android-sdk"
require_dir_env "ANDROID_NDK_ROOT" "\$ANDROID_SDK_ROOT/ndk/28.2.13676358"

# Show build config
if [ "$BUILD_ARM32" = true ]; then
    echo "✅ 32-bit (armeabi-v7a)"
else
    echo "✅ 64-bit (arm64-v8a)"
fi
echo "✅ App minSdk: $MIN_SDK"

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

    case "$target" in
        aarch64-linux-android)
            export AR_aarch64_linux_android="$ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/darwin-x86_64/bin/llvm-ar"
            export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/darwin-x86_64/bin/$cc_bin"
            export CC_aarch64_linux_android="$ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/darwin-x86_64/bin/$cc_bin"
            export CXX_aarch64_linux_android="$ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/darwin-x86_64/bin/$cxx_bin"
            ;;
        armv7-linux-androideabi)
            export AR_armv7_linux_androideabi="$ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/darwin-x86_64/bin/llvm-ar"
            export CARGO_TARGET_ARMV7_LINUX_ANDROIDEABI_LINKER="$ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/darwin-x86_64/bin/$cc_bin"
            export CC_armv7_linux_androideabi="$ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/darwin-x86_64/bin/$cc_bin"
            export CXX_armv7_linux_androideabi="$ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/darwin-x86_64/bin/$cxx_bin"
            # Old Android (API < 23) requires DT_HASH, not just DT_GNU_HASH
            export CARGO_TARGET_ARMV7_LINUX_ANDROIDEABI_RUSTFLAGS="-C link-arg=-Wl,--hash-style=both"
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
    else
        echo "  → Building lingxia-lib ($target)..."
    fi
    run_cargo_with_lxapp_features cargo build --target $target --release -p lingxia-lib
}

echo "[0/4] Preparing Android SDK (resources + local Maven)..."
bash "$LINGXIA_ROOT/lingxia-sdk/release.sh" \
  --platform android \
  --android-maven-dir "$LOCAL_MAVEN_DIR" \
  --android-no-zip \
  --no-shasums

if [ "$SKIP_RUST" = false ]; then
    echo "[1/4] Building Rust libraries..."
    cd "$WORKSPACE_ROOT"

    if [ "$BUILD_ARM32" = true ]; then
        # Use API 21 for arm32 to support older devices (API 22+)
        build_rust_android "armv7-linux-androideabi" "armv7-a" "armv7a-linux-androideabi21-clang" "armv7a-linux-androideabi21-clang++"
        echo "  → Copying liblingxia.so to App jniLibs (armeabi-v7a)..."
        mkdir -p "$APP_JNILIBS_DIR_ARMV7"
        cp "$WORKSPACE_ROOT/target/armv7-linux-androideabi/release/liblingxia.so" "$APP_JNILIBS_DIR_ARMV7/liblingxia.so"
    else
        build_rust_android "aarch64-linux-android" "aarch64" "aarch64-linux-android33-clang" "aarch64-linux-android33-clang++"
        echo "  → Copying liblingxia.so to App jniLibs (arm64-v8a)..."
        mkdir -p "$APP_JNILIBS_DIR_ARM64"
        cp "$WORKSPACE_ROOT/target/aarch64-linux-android/release/liblingxia.so" "$APP_JNILIBS_DIR_ARM64/liblingxia.so"
    fi

    echo "✅ Rust build complete"
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
RUNTIME_TARGET="es2020"
if [ "$BUILD_ARM32" = true ]; then
    RUNTIME_TARGET="es5"
fi
build_and_copy_runtime "$ASSETS_DIR" "$RUNTIME_TARGET" "mobile"
build_and_copy_homelxapp "$ASSETS_DIR"

echo "[4/4] Building and installing Android example app (release)..."
cd "$SCRIPT_DIR"
# ./gradlew clean
./gradlew --refresh-dependencies clean assembleRelease \
    -PLINGXIA_SDK_VERSION="$LINGXIA_SDK_VERSION" \
    -PMIN_SDK="$MIN_SDK"

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
