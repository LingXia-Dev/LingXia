#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
GH_REPO="${LINGXIA_RELEASE_REPO:-LingXia-Dev/LingXia}"

RUNNER_ALL_ARCHES=(arm64 x86_64)

usage() {
  cat <<'EOF'
LingXia unified release entrypoint.

Usage:
  scripts/release/main.sh <command> [args...]

Commands:
  doctor              Show key versions, release tag, and script locations
  crates              Release crates.io packages
  npm                 Release npm packages (@lingxia/bridge/elements/react/vue/html/types/skill and internal page-runtime)
  cli                 Build/upload CLI GitHub Release assets
  runner              Build/upload Runner GitHub Release assets
  sdk                 Build/package SDK release artifacts

CLI options:
  --target <platform> Build specific target(s): darwin-x64, darwin-arm64, all
  --publish           Upload built assets to the GitHub release tag
  --tag <tag>         Release tag to upload to (default: lingxia-cli-v<version>)
  --out <dir>         Output directory (default: ./dist/cli-release)
  --skip-build        Reuse existing cargo artifacts

Runner options:
  --macos-arch <arch> Build specific Runner arch: arm64, x86_64, all
  --publish           Upload built assets to the GitHub release tag
  --tag <tag>         Release tag to upload to (default: lingxia-cli-v<version>)
  --out <dir>         Output directory (default: ./dist/runner-release)
  --skip-build        Reuse existing runner build artifacts

SDK options:
  --platform <name>   apple/ios, android, harmony, or all
  --gh-upload         Upload built SDK assets to the GitHub release tag
  --tag <tag>         Release tag to upload to (default: lingxia-sdk-v<version>)

Examples:
  scripts/release/main.sh doctor
  scripts/release/main.sh crates --dry-run
  scripts/release/main.sh npm --package all --publish
  scripts/release/main.sh cli --target all --publish
  scripts/release/main.sh runner --publish
  scripts/release/main.sh sdk --platform all --gh-upload
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

release_tag_for_version() {
  local version="$1"
  printf 'lingxia-cli-v%s\n' "$version"
}

current_cli_target() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Darwin) os="darwin" ;;
    *)
      echo "ERROR: unsupported CLI host OS: $os" >&2
      return 2
      ;;
  esac

  case "$arch" in
    x86_64|amd64) arch="x64" ;;
    arm64|aarch64) arch="arm64" ;;
    *)
      echo "ERROR: unsupported CLI host arch: $arch" >&2
      return 2
      ;;
  esac

  printf '%s-%s\n' "$os" "$arch"
}

doctor() {
  local ws_v cli_asset cli_runner_tag sdk_tag bridge_v elements_v react_v vue_v html_v page_runtime_v types_v cli_target
  ws_v="$(workspace_version)"
  if cli_target="$(current_cli_target 2>/dev/null)"; then
    cli_asset="lingxia-$cli_target"
  else
    cli_asset="N/A (unsupported host)"
  fi
  cli_runner_tag="$(release_tag_for_version "$ws_v")"
  sdk_tag="lingxia-sdk-v$ws_v"
  bridge_v="$(node -p "require('$ROOT_DIR/packages/lingxia-bridge/package.json').version" 2>/dev/null || echo "N/A")"
  elements_v="$(node -p "require('$ROOT_DIR/packages/lingxia-elements/package.json').version" 2>/dev/null || echo "N/A")"
  react_v="$(node -p "require('$ROOT_DIR/packages/lingxia-react/package.json').version" 2>/dev/null || echo "N/A")"
  vue_v="$(node -p "require('$ROOT_DIR/packages/lingxia-vue/package.json').version" 2>/dev/null || echo "N/A")"
  html_v="$(node -p "require('$ROOT_DIR/packages/lingxia-html/package.json').version" 2>/dev/null || echo "N/A")"
  page_runtime_v="$(node -p "require('$ROOT_DIR/packages/lingxia-page-runtime/package.json').version" 2>/dev/null || echo "N/A")"
  types_v="$(node -p "require('$ROOT_DIR/packages/lingxia-types/package.json').version" 2>/dev/null || echo "N/A")"
  skill_v="$(node -p "require('$ROOT_DIR/packages/lingxia-skill/package.json').version" 2>/dev/null || echo "N/A")"

  cat <<EOF
Workspace version:      $ws_v
CLI release tag:        $cli_runner_tag
CLI current asset:      $cli_asset
Runner release tag:     $cli_runner_tag
Runner arches:          ${RUNNER_ALL_ARCHES[*]}
SDK release tag:        $sdk_tag
NPM bridge version:     $bridge_v
NPM elements version:   $elements_v
NPM react version:      $react_v
NPM vue version:        $vue_v
NPM html version:       $html_v
NPM page-runtime version: $page_runtime_v
NPM types version:      $types_v
NPM skill version:      $skill_v
GitHub release repo:    $GH_REPO

Scripts:
  main:   scripts/release/main.sh
  crates: scripts/release/crates.sh
  npm:    scripts/release/npm.sh
  cli:    scripts/release/cli.sh
  runner: scripts/release/runner.sh
  sdk:    scripts/release/sdk.sh
  install: install.sh
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
  runner)
    run_cmd "$SCRIPT_DIR/runner.sh" "$@"
    ;;
  sdk)
    run_cmd "$SCRIPT_DIR/sdk.sh" "$@"
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
