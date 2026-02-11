#!/bin/bash
# Common utilities for dev.sh scripts
#
# Usage:
#   source "$LINGXIA_ROOT/examples/scripts/common.sh"
#   parse_common_args "$@" || handle_platform_specific_args
#   build_and_copy_homelxapp "$TARGET_DIR"

# Initialize common variables (call after setting SCRIPT_DIR)
init_common_vars() {
    LINGXIA_ROOT="${LINGXIA_ROOT:-$SCRIPT_DIR/../..}"
    LXAPP_FEATURES="${LXAPP_FEATURES:-}"
    SKIP_RUST=false
    FRAMEWORK=""
    EXPECT_FRAMEWORK=false
    CLEAN_INSTALL=false
    HOME_LXAPP="${HOME_LXAPP:-homelxapp}"  # Configurable home lxapp name
    EXPECT_LXAPP=false
}

# Ensure a default TLS feature is present in LXAPP_FEATURES.
# If user already specified tls-ring or tls-aws-lc, keep user choice.
ensure_tls_feature_default() {
    local default_tls_feature="$1"
    local features="${LXAPP_FEATURES// /}"

    if [[ "$features" =~ (^|,)tls-ring(,|$) ]] || [[ "$features" =~ (^|,)tls-aws-lc(,|$) ]]; then
        LXAPP_FEATURES="$features"
        return 0
    fi

    if [ -z "$features" ]; then
        LXAPP_FEATURES="$default_tls_feature"
    else
        LXAPP_FEATURES="$features,$default_tls_feature"
    fi
    echo "🔐 Default TLS feature enabled: $default_tls_feature"
}

# Run cargo command with LXAPP feature policy:
# - if both tls-ring and tls-aws-lc are set: fail fast
# - if only tls-ring is set: add --no-default-features to avoid default tls-aws-lc conflict
# - otherwise: pass --features as-is
run_cargo_with_lxapp_features() {
    local features="${LXAPP_FEATURES// /}"
    if [ -z "$features" ]; then
        "$@"
        return $?
    fi

    local has_ring=false
    local has_aws=false
    if [[ ",$features," == *",tls-ring,"* ]]; then
        has_ring=true
    fi
    if [[ ",$features," == *",tls-aws-lc,"* ]]; then
        has_aws=true
    fi

    if [ "$has_ring" = true ] && [ "$has_aws" = true ]; then
        echo "❌ Conflicting TLS features: tls-ring and tls-aws-lc cannot be enabled together" >&2
        return 1
    fi

    if [ "$has_ring" = true ] && [ "$has_aws" = false ]; then
        "$@" --no-default-features --features "$features"
    else
        "$@" --features "$features"
    fi
}

# Parse a single argument
# Returns 0 if handled, 1 if not recognized (let caller handle)
parse_common_arg() {
    local arg="$1"
    # Handle expected values from previous flags
    if [ "$EXPECT_FRAMEWORK" = true ]; then
        if [[ "$arg" == -* ]]; then
            echo "❌ Error: --framework requires a value (react|vue)"
            exit 1
        fi
        FRAMEWORK="$arg"
        EXPECT_FRAMEWORK=false
        echo "🎯 Framework: $FRAMEWORK"
        return 0
    fi
    if [ "$EXPECT_LXAPP" = true ]; then
        if [[ "$arg" == -* ]]; then
            echo "❌ Error: --lxapp requires a value"
            exit 1
        fi
        HOME_LXAPP="$arg"
        EXPECT_LXAPP=false
        echo "📦 Home LxApp: $HOME_LXAPP"
        return 0
    fi
    case "$arg" in
        --framework)
            EXPECT_FRAMEWORK=true
            return 0
            ;;
        react|vue)
            if [ -z "$FRAMEWORK" ]; then
                FRAMEWORK="$arg"
                echo "🎯 Framework: $FRAMEWORK"
            fi
            return 0
            ;;
        --lxapp)
            EXPECT_LXAPP=true
            return 0
            ;;
        --clean|clean)
            CLEAN_INSTALL=true
            echo "🧹 Clean install enabled"
            return 0
            ;;
        --skip-rust|skip-rust)
            SKIP_RUST=true
            echo "🚀 Skipping Rust compilation"
            return 0
            ;;
        *)
            return 1
            ;;
    esac
}

# Show help message
# Usage: show_help "platform-specific options here"
show_help() {
    local extra_options="${1:-}"
    echo "Usage: $0 [options]"
    echo "Options:"
    echo "  --framework react|vue   Specify framework (default: auto-detect)"
    echo "  --lxapp <name>          Specify home lxapp to build (default: homelxapp)"
    echo "  --clean                 Uninstall app before install"
    echo "  --skip-rust             Skip Rust compilation"
    if [ -n "$extra_options" ]; then
        echo "$extra_options"
    fi
}

# Build home lxapp and copy to target directory
# Usage: build_and_copy_homelxapp "$TARGET_DIR"
# Uses HOME_LXAPP variable (default: homelxapp)
build_and_copy_homelxapp() {
    local target_dir="$1"
    local lxapp_name="${HOME_LXAPP:-homelxapp}"
    local lxapp_dir="$LINGXIA_ROOT/examples/$lxapp_name"

    if [ ! -d "$lxapp_dir" ]; then
        echo "❌ Error: LxApp directory not found: $lxapp_dir"
        exit 1
    fi

    echo "Building and copying $lxapp_name..."
    cd "$lxapp_dir"

    if [ -n "$FRAMEWORK" ]; then
        echo "  → Building with framework: $FRAMEWORK"
        npm run build:$FRAMEWORK
    else
        echo "  → Building with auto-detected framework"
        npm run build
    fi

    if [ -d "dist" ]; then
        mkdir -p "$target_dir/homelxapp"
        cp -R dist/* "$target_dir/homelxapp/"
        echo "  ✅ $lxapp_name copied to $target_dir/homelxapp"
    else
        echo "❌ Error: dist directory not found in $lxapp_dir"
        exit 1
    fi
}

# Build web runtime and copy runtime.js to target directory
# Usage: build_and_copy_runtime "$TARGET_DIR" [es2020|es5] [all|desktop|mobile]
build_and_copy_runtime() {
    local target_dir="$1"
    local ecma_target="${2:-es2020}"
    local runtime_platform="${3:-all}"
    local runtime_dir="$LINGXIA_ROOT/lingxia-web-runtime"
    local dist_runtime=""
    local build_script="build"

    case "$ecma_target" in
        es2020)
            build_script="build:es2020"
            dist_runtime="$runtime_dir/dist/runtime.es2020.js"
            ;;
        es5)
            build_script="build:es5"
            dist_runtime="$runtime_dir/dist/runtime.es5.js"
            ;;
        *)
            echo "❌ Error: Unsupported runtime target '$ecma_target' (expected es2020 or es5)"
            exit 1
            ;;
    esac

    case "$runtime_platform" in
        all|desktop|mobile) ;;
        *)
            echo "❌ Error: Unsupported runtime platform '$runtime_platform' (expected all, desktop, or mobile)"
            exit 1
            ;;
    esac

    if [ ! -f "$runtime_dir/package.json" ]; then
        echo "❌ Error: Runtime package not found: $runtime_dir/package.json"
        exit 1
    fi
    if [ ! -d "$runtime_dir/node_modules" ]; then
        echo "❌ Error: Missing $runtime_dir/node_modules (run: cd $runtime_dir && npm ci)"
        exit 1
    fi

    echo "Building web runtime ($ecma_target, platform=$runtime_platform)..."
    if [ "$runtime_platform" = "all" ]; then
        (cd "$runtime_dir" && npm run "$build_script")
    else
        (cd "$runtime_dir" && LX_RUNTIME_PLATFORM="$runtime_platform" npm run "$build_script")
    fi

    if [ ! -f "$dist_runtime" ]; then
        echo "❌ Error: runtime.js not found after build: $dist_runtime"
        exit 1
    fi

    mkdir -p "$target_dir"
    cp "$dist_runtime" "$target_dir/runtime.js"
    echo "  ✅ runtime.js copied to $target_dir/runtime.js"
}

# Generate app.json configuration
# Usage: generate_app_config "$TARGET_DIR"
generate_app_config() {
    local target_dir="$1"
    echo "Generating host app configuration..."
    source "$LINGXIA_ROOT/examples/scripts/generate-app-json.sh"
    generate_app_json "$target_dir"
}
