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
#   LINGXIA_API_KEY    - API key to include in app.json
#   LINGXIA_API_SECRET - API secret to include in app.json
#
# Optional secrets file:
#   $LINGXIA_ROOT/examples/.lingxia.secrets.json
#   Supports: apiServer, apiKey, apiSecret

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

    local home_lxapp_id
    home_lxapp_id=$(jq -r '.app.homeLxAppID // empty' "$config_file")
    if [ -z "$home_lxapp_id" ]; then
        echo "Error: app.homeLxAppID missing in $config_file" >&2
        return 1
    fi

    local home_lxapp_json="$LINGXIA_ROOT/examples/$home_lxapp_id/lxapp.json"
    if [ ! -f "$home_lxapp_json" ]; then
        echo "Error: home lxapp config not found: $home_lxapp_json" >&2
        return 1
    fi

    local home_lxapp_version
    home_lxapp_version=$(jq -r '.version // empty' "$home_lxapp_json")
    if [ -z "$home_lxapp_version" ]; then
        echo "Error: version missing in $home_lxapp_json" >&2
        return 1
    fi

    local base_json
    base_json=$(jq --arg home_lxapp_id "$home_lxapp_id" --arg home_lxapp_version "$home_lxapp_version" '{
        productName: .app.productName,
        productVersion: .app.productVersion,
        homeLxAppID: $home_lxapp_id,
        homeLxAppVersion: $home_lxapp_version
    } + (if .app.cacheMaxAgeDays then {cacheMaxAgeDays: .app.cacheMaxAgeDays} else {} end)' "$config_file")

    local config_api_server
    config_api_server=$(jq -r '.app.apiServer // empty' "$config_file")

    local secrets_file="$LINGXIA_ROOT/examples/.lingxia.secrets.json"
    local secrets_api_server=""
    local secrets_api_key=""
    local secrets_api_secret=""
    if [ -f "$secrets_file" ]; then
        if ! jq -e . "$secrets_file" > /dev/null 2>&1; then
            echo "Error: Invalid JSON in $secrets_file" >&2
            return 1
        fi
        secrets_api_server=$(jq -r '.apiServer // empty' "$secrets_file")
        secrets_api_key=$(jq -r '.apiKey // empty' "$secrets_file")
        secrets_api_secret=$(jq -r '.apiSecret // empty' "$secrets_file")
    fi

    local effective_api_server="$config_api_server"
    local effective_api_key="$secrets_api_key"
    local effective_api_secret="$secrets_api_secret"

    # Priority: environment variables > secrets file > config file
    if [ -n "${LINGXIA_API_SERVER:-}" ]; then
        effective_api_server="$LINGXIA_API_SERVER"
    elif [ -n "$secrets_api_server" ]; then
        effective_api_server="$secrets_api_server"
    fi
    if [ -n "${LINGXIA_API_KEY:-}" ]; then
        effective_api_key="$LINGXIA_API_KEY"
    fi
    if [ -n "${LINGXIA_API_SECRET:-}" ]; then
        effective_api_secret="$LINGXIA_API_SECRET"
    fi

    if [ -n "$effective_api_server" ]; then
        base_json=$(echo "$base_json" | jq --arg server "$effective_api_server" '. + {apiServer: $server}')
    fi
    if [ -n "$effective_api_key" ]; then
        base_json=$(echo "$base_json" | jq --arg key "$effective_api_key" '. + {apiKey: $key}')
    fi
    if [ -n "$effective_api_secret" ]; then
        base_json=$(echo "$base_json" | jq --arg secret "$effective_api_secret" '. + {apiSecret: $secret}')
    fi

    echo "$base_json" > "$output_dir/app.json"

    echo "✅ Generated app.json in $output_dir"
}
