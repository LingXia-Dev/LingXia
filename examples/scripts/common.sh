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
}

# Parse a single argument
# Returns 0 if handled, 1 if not recognized (let caller handle)
parse_common_arg() {
    local arg="$1"
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
    case "$arg" in
        --framework=*)
            FRAMEWORK="${arg#*=}"
            echo "🎯 Framework: $FRAMEWORK"
            return 0
            ;;
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
    echo "  --clean                 Uninstall app before install"
    echo "  --skip-rust             Skip Rust compilation"
    if [ -n "$extra_options" ]; then
        echo "$extra_options"
    fi
}

# Build homelxapp and copy to target directory
# Usage: build_and_copy_homelxapp "$TARGET_DIR"
build_and_copy_homelxapp() {
    local target_dir="$1"
    
    echo "Building and copying homelxapp..."
    cd "$LINGXIA_ROOT/examples/homelxapp"
    
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
        echo "  ✅ homelxapp copied to $target_dir/homelxapp"
    else
        echo "❌ Error: dist directory not found"
        exit 1
    fi
}

# Generate app.json configuration
# Usage: generate_app_config "$TARGET_DIR"
generate_app_config() {
    local target_dir="$1"
    echo "Generating host app configuration..."
    source "$LINGXIA_ROOT/examples/scripts/generate-app-json.sh"
    generate_app_json "$target_dir"
}
