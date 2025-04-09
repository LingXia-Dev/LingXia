#!/bin/bash

# Exit on error
set -e

# Get the absolute path of the script directory
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$SCRIPT_DIR/../.."

echo "Building Rust library..."
cd "$PROJECT_ROOT"
env \
AR_aarch64_linux_android="$ANDROID_NDK/toolchains/llvm/prebuilt/darwin-x86_64/bin/llvm-ar" \
CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$ANDROID_NDK/toolchains/llvm/prebuilt/darwin-x86_64/bin/aarch64-linux-android33-clang" \
cargo build --target aarch64-linux-android --release

echo "Copying Rust library to jniLibs..."
JNILIBS_DIR="$PROJECT_ROOT/android/lingxia/src/main/jniLibs/arm64-v8a"
mkdir -p "$JNILIBS_DIR"
cp "$PROJECT_ROOT/target/aarch64-linux-android/release/liblingxia.so" "$JNILIBS_DIR/"

echo "Building Android library..."
cd "$PROJECT_ROOT/android"
./gradlew :lingxia:clean
./gradlew :lingxia:assembleDebug

echo "Building and installing Android app..."
cd "$SCRIPT_DIR"
./gradlew clean
./gradlew assembleDebug
adb install -r ./app/build/outputs/apk/debug/app-debug.apk
