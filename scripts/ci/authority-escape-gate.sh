#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FIXTURE_MANIFEST="$ROOT_DIR/crates/lingxia/tests/fixtures/authority-escape/Cargo.toml"
FIXTURE_LOCK="${FIXTURE_MANIFEST%/*}/Cargo.lock"

BASE_EXPECTATIONS=(
  'E0603|impl rong_command::ProcessAuthority'
  'E0425|let _ = lxapp::__init_with_native_authority;'
  'E0603|let _ = lxapp::terminal_automation::NativeHostRuntimeToken::for_test;'
  'E0599|let _ = lxapp::NativeControlPlaneAuthority::for_test;'
  'E0624|let _ = lxapp::NativeControlPlaneAuthority::for_native_runtime;'
  'E0425|let _ = lxapp::host::__install_app_resource_grant_resolver;'
  'E0425|let _ = lxapp::host::__install_devtools_resource_grant_resolver;'
  'E0599|let _ = lxapp::host::AuthenticatedCaller::LxAppSession;'
  'E0599|let _ = lxapp::host::AuthenticatedCaller::BrowserDocument;'
  'E0425|let _ = lxapp::add_global_page_script;'
  'E0599|let _ = lxapp::LxApp::add_page_script;'
  'E0425|let _ = lingxia::__init_with_native_authority;'
  'E0425|let _ = lingxia::resolve_settings_destination;'
  'E0603|let _ = rong_command::init;'
  'E0603|let _ = rong_command::init_with_authority;'
)
APPLE_EXPECTATION='E0603|let _ = lingxia::apple::resolve_settings_destination_for_host;'

diagnostic_has() {
  local output_file="$1"
  local code="$2"
  local needle="$3"
  awk -v header="error[$code]" -v needle="$needle" '
    /^error(\[[A-Z][0-9]+\])?:/ {
      if (active && found) {
        exit 0
      }
      active = index($0, header) != 0
      found = 0
      next
    }
    active && index($0, needle) != 0 { found = 1 }
    END { exit(active && found ? 0 : 1) }
  ' "$output_file"
}

verify_result() {
  local status="$1"
  local output_file="$2"
  local require_apple="$3"
  local expectations=("${BASE_EXPECTATIONS[@]}")
  if [[ "$require_apple" == true ]]; then
    expectations+=("$APPLE_EXPECTATION")
  fi

  if [[ "$status" -eq 0 ]]; then
    echo "authority escape fixture compiled successfully" >&2
    return 1
  fi

  local expectation code needle
  for expectation in "${expectations[@]}"; do
    code="${expectation%%|*}"
    needle="${expectation#*|}"
    if ! diagnostic_has "$output_file" "$code" "$needle"; then
      echo "missing authority rejection $code for: $needle" >&2
      return 1
    fi
  done
}

emit_synthetic_diagnostics() {
  local require_apple="$1"
  local skip_first="${2:-false}"
  local expectations=("${BASE_EXPECTATIONS[@]}")
  if [[ "$require_apple" == true ]]; then
    expectations+=("$APPLE_EXPECTATION")
  fi

  local index=0 expectation code needle
  for expectation in "${expectations[@]}"; do
    index=$((index + 1))
    if [[ "$skip_first" == true && "$index" -eq 1 ]]; then
      continue
    fi
    code="${expectation%%|*}"
    needle="${expectation#*|}"
    printf 'error[%s]: authority must stay private\n  |\n1 | %s\n\n' "$code" "$needle"
  done
}

self_test() {
  local temp_dir good missing
  temp_dir="$(mktemp -d)"
  good="$temp_dir/good.log"
  missing="$temp_dir/missing.log"
  trap 'rm -rf "$temp_dir"' RETURN

  emit_synthetic_diagnostics true >"$good"
  verify_result 101 "$good" true
  if verify_result 0 "$good" true >/dev/null 2>&1; then
    echo "authority gate accepted a successful compile" >&2
    return 1
  fi

  emit_synthetic_diagnostics true true >"$missing"
  if verify_result 101 "$missing" true >/dev/null 2>&1; then
    echo "authority gate accepted a missing diagnostic" >&2
    return 1
  fi
}

run_gate() {
  local temp_dir output status require_apple had_lock
  temp_dir="$(mktemp -d)"
  output="$temp_dir/cargo-check.log"
  had_lock=false
  [[ -e "$FIXTURE_LOCK" ]] && had_lock=true
  trap 'rm -rf "$temp_dir"; if [[ "$had_lock" == false ]]; then rm -f "$FIXTURE_LOCK"; fi' RETURN

  set +e
  CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT_DIR/target}" \
    cargo check \
      --manifest-path "$FIXTURE_MANIFEST" \
      --all-features \
      --color never >"$output" 2>&1
  status=$?
  set -e

  require_apple=false
  [[ "$(uname -s)" == Darwin ]] && require_apple=true
  if ! verify_result "$status" "$output" "$require_apple"; then
    cat "$output" >&2
    return 1
  fi
  echo "authority escape fixture rejected every forbidden safe API"
}

case "${1:-run}" in
  run) run_gate ;;
  self-test) self_test ;;
  *) echo "usage: $0 [run|self-test]" >&2; exit 2 ;;
esac
