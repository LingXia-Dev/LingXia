#!/usr/bin/env bash

set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)

assert_case() {
  local name="$1"
  local expected_automation="$2"
  local expected_matrix="$3"
  shift 3

  local output
  output=$(env \
    FULL=false \
    CROSS_PLATFORM=false \
    MACOS=false \
    WINDOWS=false \
    MACOS_ALL=false \
    WINDOWS_ALL=false \
    FRONTEND_SHARED=false \
    REACT=false \
    "$@" \
    bash "$script_dir/automation-matrix.sh")

  local actual_automation
  actual_automation=$(sed -n 's/^automation=//p' <<<"$output")
  local actual_matrix
  actual_matrix=$(sed -n 's/^automation_matrix=//p' <<<"$output")

  if [[ "$actual_automation" != "$expected_automation" || "$actual_matrix" != "$expected_matrix" ]]; then
    echo "matrix case failed: $name" >&2
    echo "expected automation=$expected_automation matrix=$expected_matrix" >&2
    echo "actual   automation=$actual_automation matrix=$actual_matrix" >&2
    return 1
  fi
}

windows='{"platform":"windows","os":"windows-latest","exe":".exe","profile":"showcase"}'
macos='{"platform":"macos","os":"macos-latest","exe":"","profile":"showcase"}'

assert_case none false \
  '{"include":[{"platform":"windows","os":"windows-latest","exe":".exe","profile":"skipped"}]}'
assert_case cross-platform true \
  "{\"include\":[$windows,$macos]}" CROSS_PLATFORM=true
assert_case macos-contract-change true \
  "{\"include\":[$macos]}" MACOS=true
assert_case macos-all true \
  "{\"include\":[$macos]}" MACOS_ALL=true
assert_case windows-contract-change true \
  "{\"include\":[$windows]}" WINDOWS=true
assert_case windows-all true \
  "{\"include\":[$windows]}" WINDOWS_ALL=true
assert_case shared-frontend true \
  "{\"include\":[$windows,$macos]}" FRONTEND_SHARED=true
assert_case react true \
  "{\"include\":[$windows,$macos]}" REACT=true
assert_case full true \
  "{\"include\":[$windows,$macos]}" FULL=true

echo "automation matrix cases passed"
