#!/usr/bin/env bash

set -euo pipefail

framework=${1:-all}
timeout_seconds=${2:-300}
repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
showcase_root="$repo_root/examples/lingxia-showcase"
lxapp_root="$showcase_root/lxapp"
lingxia="$repo_root/target/debug/lingxia"
lxdev="$repo_root/target/debug/lxdev"

case "$framework" in
  react|vue) frameworks=("$framework") ;;
  all) frameworks=(react vue) ;;
  *) echo "Unsupported framework: $framework" >&2; exit 2 ;;
esac

cleanup() {
  (cd "$showcase_root" && "$lingxia" dev stop macos) >/dev/null 2>&1 || true
}
trap cleanup EXIT

echo 'Resolving automation CLIs (cache / matching release / cargo build)...'
bash "$repo_root/scripts/ci/resolve-cli.sh" --dest "$repo_root/target/debug"
(cd "$showcase_root" && "$lingxia" doctor --platform macos)

for framework_index in "${!frameworks[@]}"; do
  current_framework=${frameworks[$framework_index]}
  dev_args=(dev --background --platform macos --framework "$current_framework")
  if (( framework_index > 0 )); then
    # Both renderers use the same native host; only restage the Vue lxapp.
    dev_args+=(--skip-native)
  fi

  echo "Starting macOS Showcase ($current_framework)..."
  (cd "$showcase_root" && "$lingxia" "${dev_args[@]}")
  ready_deadline=$((SECONDS + 1800))
  ready=false
  while (( SECONDS < ready_deadline )); do
    status_json=$(cd "$showcase_root" && "$lingxia" dev status --json)
    if grep -Eq '"runtime_connected"[[:space:]]*:[[:space:]]*true' <<<"$status_json"; then
      ready=true
      break
    fi
    sleep 5
  done
  if [[ "$ready" != true ]]; then
    echo 'macOS dev session did not become ready within 1800 seconds.' >&2
    exit 1
  fi
  echo "$status_json"

  result_dir="$lxapp_root/test-results/automation/macos-$current_framework"
  mkdir -p "$result_dir"
  echo "Running macOS Showcase automation ($current_framework)..."
  set +e
  (
    cd "$lxapp_root"
    "$lxdev" test tests/entries/macos.test.ts \
      --timeout "$timeout_seconds" \
      --arg platform=macos \
      --arg "framework=$current_framework" \
      --output-dir "test-results/automation/macos-$current_framework"
  )
  test_status=$?
  set -e

  (cd "$showcase_root" && "$lxdev" logs --json --limit 5000) > "$result_dir/session.jsonl"
  error_logs=$(cd "$showcase_root" && "$lxdev" logs --level error --json --limit 1000)
  if [[ -n "$error_logs" ]]; then
    echo "Unexpected error-level macOS session logs:" >&2
    echo "$error_logs" >&2
    exit 1
  fi
  if (( test_status != 0 )); then
    exit "$test_status"
  fi

  echo "Stopping macOS Showcase ($current_framework)..."
  (cd "$showcase_root" && "$lingxia" dev stop macos)
done

trap - EXIT
echo 'macOS Showcase automation passed.'
