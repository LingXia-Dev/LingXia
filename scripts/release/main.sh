#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

usage() {
  cat <<'EOF'
LingXia unified release entrypoint.

Usage:
  scripts/release/main.sh <command> [args...]

Commands:
  doctor              Show key versions and release script locations
  crates              Release crates.io packages
  npm                 Release npm packages (@lingxia/bridge/elements/react/vue/types)
  cli                 Release @lingxia/cli packages
  sdk                 Build/package SDK release artifacts
  all [--publish]     Run crates -> npm -> cli -> sdk in order

Examples:
  scripts/release/main.sh doctor
  scripts/release/main.sh crates --dry-run
  scripts/release/main.sh npm --package all --publish
  scripts/release/main.sh cli --target all --publish
  scripts/release/main.sh sdk --platform all
EOF
}

workspace_version() {
  awk '
    /^\[workspace\.package\]/ {in_section=1; next}
    /^\[/ {in_section=0}
    in_section && $1 == "version" {
      gsub(/"/, "", $3);
      print $3;
      exit
    }' "$ROOT_DIR/Cargo.toml"
}

doctor() {
  local ws_v cli_v bridge_v elements_v react_v vue_v types_v
  ws_v="$(workspace_version)"
  cli_v="$(node -p "require('$ROOT_DIR/tools/lingxia-cli/npm/package.json').version" 2>/dev/null || echo "N/A")"
  bridge_v="$(node -p "require('$ROOT_DIR/packages/lingxia-bridge/package.json').version" 2>/dev/null || echo "N/A")"
  elements_v="$(node -p "require('$ROOT_DIR/packages/lingxia-elements/package.json').version" 2>/dev/null || echo "N/A")"
  react_v="$(node -p "require('$ROOT_DIR/packages/lingxia-react/package.json').version" 2>/dev/null || echo "N/A")"
  vue_v="$(node -p "require('$ROOT_DIR/packages/lingxia-vue/package.json').version" 2>/dev/null || echo "N/A")"
  types_v="$(node -p "require('$ROOT_DIR/packages/lingxia-types/package.json').version" 2>/dev/null || echo "N/A")"

  cat <<EOF
Workspace version:    $ws_v
CLI npm version:      $cli_v
NPM bridge version:   $bridge_v
NPM elements version: $elements_v
NPM react version:    $react_v
NPM vue version:      $vue_v
NPM types version:    $types_v

Scripts:
  crates: scripts/release/crates.sh
  npm:    scripts/release/npm.sh
  cli:    scripts/release/cli.sh
  sdk:    scripts/release/sdk.sh
EOF
}

run_cmd() {
  local cmd="$1"
  shift
  "$cmd" "$@"
}

cmd="${1:-}"
if [[ -z "$cmd" ]]; then
  usage
  exit 2
fi
shift || true

case "$cmd" in
  doctor)
    doctor
    ;;
  crates)
    run_cmd "$SCRIPT_DIR/crates.sh" "$@"
    ;;
  npm)
    run_cmd "$SCRIPT_DIR/npm.sh" "$@"
    ;;
  cli)
    run_cmd "$SCRIPT_DIR/cli.sh" "$@"
    ;;
  sdk)
    run_cmd "$SCRIPT_DIR/sdk.sh" "$@"
    ;;
  all)
    all_publish=0
    if [[ "${1:-}" == "--publish" ]]; then
      all_publish=1
      shift
    fi

    if [[ $# -gt 0 ]]; then
      echo "Unknown option(s) for 'all': $*" >&2
      usage
      exit 2
    fi

    if [[ "$all_publish" -eq 1 ]]; then
      run_cmd "$SCRIPT_DIR/crates.sh" --publish
      run_cmd "$SCRIPT_DIR/npm.sh" --package all --publish
      run_cmd "$SCRIPT_DIR/sdk.sh" --platform all --gh-upload
      run_cmd "$SCRIPT_DIR/cli.sh" --target all --publish
    else
      run_cmd "$SCRIPT_DIR/crates.sh" --dry-run --allow-dirty
      run_cmd "$SCRIPT_DIR/npm.sh" --package all --dry-run
      run_cmd "$SCRIPT_DIR/cli.sh" --target all
      run_cmd "$SCRIPT_DIR/sdk.sh" --platform all
    fi
    ;;
  -h|--help|help)
    usage
    ;;
  *)
    echo "Unknown command: $cmd" >&2
    usage
    exit 2
    ;;
esac
