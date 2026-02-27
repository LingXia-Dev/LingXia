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

# Mobile builds default to ring unless TLS backend is explicitly chosen.
ensure_tls_feature_default "tls-ring"

# Define the resources directory for iOS
RESOURCES_DIR="$SCRIPT_DIR/Sources/lxapp/Resources"
echo "RESOURCES_DIR: $RESOURCES_DIR"

# Generate Swift bridge bindings before staging the Apple SDK into target/spm/lingxia.
# This creates/updates: lingxia-sdk/apple/Sources/generated/...
if [ "$SKIP_RUST" = false ]; then
    echo "[0/4] Generating Swift bridge bindings..."
    cd "$WORKSPACE_ROOT"

    TARGET="aarch64-apple-ios"
    # `lingxia` only accepts TLS features; filter out extension features such as `cloud`
    # that belong to `lingxia-lib`.
    BRIDGE_LXAPP_FEATURES="$(
        printf '%s' "$LXAPP_FEATURES" \
            | tr ',' '\n' \
            | sed 's/[[:space:]]//g' \
            | awk '$0=="tls-ring" || $0=="tls-aws-lc"' \
            | paste -sd, -
    )"
    if [ -n "$LXAPP_FEATURES" ] && [ "$BRIDGE_LXAPP_FEATURES" != "${LXAPP_FEATURES// /}" ]; then
        echo "  → Bridge build features filtered for lingxia: ${BRIDGE_LXAPP_FEATURES:-<none>}"
    fi

    BRIDGE_LOG="$(mktemp -t lingxia_ios_bridge.XXXXXX)"
    if (
        LXAPP_FEATURES="$BRIDGE_LXAPP_FEATURES"
        run_cargo_with_lxapp_features env LINGXIA_GENERATE_BRIDGE=1 cargo build -p lingxia --target $TARGET --release
    ) >"$BRIDGE_LOG" 2>&1; then
        grep -E "Generated|warning:" "$BRIDGE_LOG" | head -5 || true
    else
        cat "$BRIDGE_LOG" >&2
        rm -f "$BRIDGE_LOG"
        exit 1
    fi
    rm -f "$BRIDGE_LOG"
fi

echo "[1/4] Preparing iOS SDK resources..."
# For dev: generate SDK i18n/icons resources via the unified SDK script.
SKIP_RUST=$SKIP_RUST bash "$LINGXIA_ROOT/lingxia-sdk/release.sh" \
  --platform apple \
  --apple-no-zip \
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
    else
        echo "  → Building lingxia-lib (staticlib)..."
    fi
    run_cargo_with_lxapp_features cargo rustc --crate-type=staticlib --target $TARGET --release -p lingxia-lib

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
build_and_copy_runtime "$RESOURCES_DIR" "es2020" "mobile"
build_and_copy_homelxapp "$RESOURCES_DIR"

echo "Building and deploying iOS app..."
cd "$SCRIPT_DIR"
env LINGXIA_PROJECT_ROOT=$LINGXIA_ROOT xtool dev
