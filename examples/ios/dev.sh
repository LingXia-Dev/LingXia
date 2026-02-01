#!/bin/bash

# Dev build & deploy LingXia Example iOS App

set -euo pipefail

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
source "$SCRIPT_DIR/../scripts/common.sh"
init_common_vars
WORKSPACE_ROOT="$LINGXIA_ROOT"

# Parse command line arguments
for arg in "$@"; do
    if ! parse_common_arg "$arg"; then
        case "$arg" in
            --help|-h)
                show_help
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

# Define the resources directory for iOS
RESOURCES_DIR="$SCRIPT_DIR/lxapp/Sources/lxapp/Resources"
echo "RESOURCES_DIR: $RESOURCES_DIR"

# Generate Swift bridge bindings before staging the Apple SDK into target/spm/lingxia.
# This creates/updates: lingxia-sdk/apple/Sources/generated/...
if [ "$SKIP_RUST" = false ]; then
    echo "[0/4] Generating Swift bridge bindings..."
    cd "$WORKSPACE_ROOT"

    TARGET="aarch64-apple-ios"
    LINGXIA_GENERATE_BRIDGE=1 cargo build -p lingxia --target $TARGET --release 2>&1 | grep -E "Generated|warning:" | head -5 || true
fi

echo "[1/4] Preparing iOS SDK resources..."
# For dev: generate Resources/icons/runtime assets via the unified SDK script.
bash "$LINGXIA_ROOT/lingxia-sdk/release.sh" \
  --platform ios \
  --ios-no-zip \
  --no-shasums \
  --out "$LINGXIA_ROOT/target/sdk-dev"

# Build Rust library for iOS unless skip-rust flag is set
if [ "$SKIP_RUST" = false ]; then
    echo "[2/4] Building Rust libraries..."
    cd "$WORKSPACE_ROOT"

    # Build lingxia-lib as staticlib for iOS (native library + user extensions)
    # Note: iOS requires staticlib (.a), not cdylib (.dylib)
    if [ -n "$LXAPP_FEATURES" ]; then
        echo "  → Building lingxia-lib (staticlib) with features: $LXAPP_FEATURES"
        cargo rustc --crate-type=staticlib --target $TARGET --release -p lingxia-lib --features "$LXAPP_FEATURES"
    else
        echo "  → Building lingxia-lib (staticlib)..."
        cargo rustc --crate-type=staticlib --target $TARGET --release -p lingxia-lib
    fi

    # Copy to expected name (liblingxia.a) for Xcode project compatibility
    cp "$WORKSPACE_ROOT/target/$TARGET/release/liblingxia_lib.a" "$WORKSPACE_ROOT/target/$TARGET/release/liblingxia.a"

    echo "✅ Rust build complete"
    echo "   .a location: $WORKSPACE_ROOT/target/$TARGET/release/liblingxia.a"
else
    echo "⏭️  Skipping Rust compilation (using existing libraries)"
fi

mkdir -p "$RESOURCES_DIR"
rm -rf "$RESOURCES_DIR"/*

generate_app_config "$RESOURCES_DIR"
build_and_copy_homelxapp "$RESOURCES_DIR"

echo "Building and deploying iOS app..."
cd "$SCRIPT_DIR/lxapp"
env LINGXIA_PROJECT_ROOT=$LINGXIA_ROOT xtool dev
