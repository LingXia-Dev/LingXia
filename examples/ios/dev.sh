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
RESOURCES_DIR="$SCRIPT_DIR/Sources/lxapp/Resources"
echo "RESOURCES_DIR: $RESOURCES_DIR"

# Generate Swift bridge bindings before staging the Apple SDK into target/spm/lingxia.
# This creates/updates: lingxia-sdk/apple/Sources/generated/...
if [ "$SKIP_RUST" = false ]; then
    TARGET="aarch64-apple-ios"
    generate_apple_swift_bridges "$TARGET" "[0/5]" "$WORKSPACE_ROOT"
fi

echo "[1/5] Preparing iOS SDK resources..."
# For dev: generate SDK i18n/icons resources via the unified SDK script.
SKIP_RUST=$SKIP_RUST bash "$LINGXIA_ROOT/scripts/release/sdk.sh" \
  --platform apple \
  --apple-no-zip \
  --no-shasums \
  --out "$LINGXIA_ROOT/target/sdk-dev"

# Build Rust library for iOS unless skip-rust flag is set
if [ "$SKIP_RUST" = false ]; then
    echo "[2/5] Building Rust libraries..."
    cd "$WORKSPACE_ROOT"
    reset_standalone_lockfile "$LINGXIA_ROOT/examples/lingxia-lib/Cargo.toml"

    # Build lingxia-lib as staticlib for iOS (native library + user extensions)
    # Note: iOS requires staticlib (.a), not cdylib (.dylib)
    if [ -n "$LXAPP_FEATURES" ]; then
        echo "  → Building lingxia-lib (staticlib) with features: $LXAPP_FEATURES"
    else
        echo "  → Building lingxia-lib (staticlib)..."
    fi
    run_cargo_with_lxapp_features cargo rustc --manifest-path "$LINGXIA_ROOT/examples/lingxia-lib/Cargo.toml" --crate-type=staticlib --target $TARGET --release

    LIB_DIR="$WORKSPACE_ROOT/target/$TARGET/release"
    LIB_PATH="$LIB_DIR/liblingxia.a"
    LEGACY_LIB_PATH="$LIB_DIR/liblingxiab.a"

    # Current crate output already uses liblingxia.a; keep legacy fallback for older local artifacts.
    if [ -f "$LIB_PATH" ]; then
        :
    elif [ -f "$LEGACY_LIB_PATH" ]; then
        cp "$LEGACY_LIB_PATH" "$LIB_PATH"
    else
        echo "❌ Missing iOS static library: neither $LIB_PATH nor $LEGACY_LIB_PATH exists" >&2
        exit 1
    fi

    echo "✅ Rust build complete"
    echo "   .a location: $LIB_PATH"

else
    echo "⏭️  Skipping Rust compilation (using existing libraries)"
fi

echo "[3/5] Preparing app resources..."
mkdir -p "$RESOURCES_DIR"
rm -rf "$RESOURCES_DIR"/*

generate_app_config "$RESOURCES_DIR"
build_and_copy_runtime "$RESOURCES_DIR" "es2020" "mobile"
build_and_copy_homelxapp "$RESOURCES_DIR"

echo "[4/5] Resetting SwiftPM build artifacts..."
cd "$SCRIPT_DIR"
rm -rf .build

echo "[5/5] Building and deploying iOS app..."
cd "$SCRIPT_DIR"
env LINGXIA_PROJECT_ROOT=$LINGXIA_ROOT xtool dev
