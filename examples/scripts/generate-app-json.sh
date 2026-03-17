#!/bin/bash

# Generate app.json from lingxia.config.json
#
# Usage: source generate-app-json.sh
#        generate_app_json <output_dir>
#
# This script reads from $LINGXIA_ROOT/examples/lingxia.config.json
# and generates app.json in the specified output directory.
#
# Required environment:
#   LINGXIA_ROOT - path to LingXia repository root
#
# Optional environment
#   LINGXIA_API_SERVER - API server to include in app.json
#
generate_app_json() {
    local output_dir="$1"

    if [ -z "$output_dir" ]; then
        echo "Usage: generate_app_json <output_dir>" >&2
        return 1
    fi

    if [ -z "$LINGXIA_ROOT" ]; then
        echo "Error: LINGXIA_ROOT not set" >&2
        return 1
    fi

    local config_file="$LINGXIA_ROOT/examples/lingxia.config.json"

    if [ ! -f "$config_file" ]; then
        echo "Error: Config file not found: $config_file" >&2
        return 1
    fi

    if ! command -v jq > /dev/null 2>&1; then
        echo "Error: jq is required to generate app.json" >&2
        return 1
    fi

    local selected_home_lxapp="${HOME_LXAPP:-}"
    local home_lxapp_id=""
    local home_lxapp_json=""
    local home_lxapp_version=""
    local lingxia_id=""

    if [ -n "$selected_home_lxapp" ]; then
        home_lxapp_json="$LINGXIA_ROOT/examples/$selected_home_lxapp/lxapp.json"
        if [ ! -f "$home_lxapp_json" ]; then
            echo "Error: HOME_LXAPP points to missing lxapp config: $home_lxapp_json" >&2
            return 1
        fi

        home_lxapp_id=$(jq -r '.appId // empty' "$home_lxapp_json")
        if [ -z "$home_lxapp_id" ]; then
            echo "Error: appId missing in $home_lxapp_json" >&2
            return 1
        fi
    else
        home_lxapp_id=$(jq -r '.app.homeLxAppID // empty' "$config_file")
        if [ -z "$home_lxapp_id" ]; then
            echo "Error: app.homeLxAppID missing in $config_file" >&2
            return 1
        fi
        home_lxapp_json="$LINGXIA_ROOT/examples/$home_lxapp_id/lxapp.json"
        if [ ! -f "$home_lxapp_json" ]; then
            echo "Error: home lxapp config not found: $home_lxapp_json" >&2
            return 1
        fi
    fi

    home_lxapp_version=$(jq -r '.version // empty' "$home_lxapp_json")
    if [ -z "$home_lxapp_version" ]; then
        echo "Error: version missing in $home_lxapp_json" >&2
        return 1
    fi

    lingxia_id=$(jq -r '.app.lingxiaId // empty' "$config_file")
    if [ -z "$lingxia_id" ]; then
        echo "Error: app.lingxiaId missing in $config_file" >&2
        return 1
    fi

    local base_json
    base_json=$(jq --arg lingxia_id "$lingxia_id" --arg home_lxapp_id "$home_lxapp_id" --arg home_lxapp_version "$home_lxapp_version" '{
        productName: .app.productName,
        productVersion: .app.productVersion,
        lingxiaId: $lingxia_id,
        homeLxAppID: $home_lxapp_id,
        homeLxAppVersion: $home_lxapp_version
    } + (if .app.cacheMaxAgeDays then {cacheMaxAgeDays: .app.cacheMaxAgeDays} else {} end)' "$config_file")

    local config_api_server
    config_api_server=$(jq -r '.app.apiServer // empty' "$config_file")

    local effective_api_server="$config_api_server"

    # Priority: environment variables > config file
    if [ -n "${LINGXIA_API_SERVER:-}" ]; then
        effective_api_server="$LINGXIA_API_SERVER"
    fi

    if [ -n "$effective_api_server" ]; then
        base_json=$(echo "$base_json" | jq --arg server "$effective_api_server" '. + {apiServer: $server}')
    fi

    # Include panels config if present
    local panels_json
    panels_json=$(jq -c '.panels // empty' "$config_file")
    if [ -n "$panels_json" ]; then
        base_json=$(echo "$base_json" | jq --argjson panels "$panels_json" '. + {panels: $panels}')
    fi

    echo "$base_json" > "$output_dir/app.json"

    echo "✅ Generated app.json in $output_dir"
}
