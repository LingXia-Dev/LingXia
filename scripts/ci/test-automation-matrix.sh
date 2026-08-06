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
    VUE=false \
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

windows_react='{"include":[{"platform":"windows","os":"windows-latest","exe":".exe","framework":"react","profile":"react"}]}'
windows_vue='{"include":[{"platform":"windows","os":"windows-latest","exe":".exe","framework":"vue","profile":"vue"}]}'
windows_both='{"include":[{"platform":"windows","os":"windows-latest","exe":".exe","framework":"react","profile":"react"},{"platform":"windows","os":"windows-latest","exe":".exe","framework":"vue","profile":"vue"}]}'

assert_case none false \
  '{"include":[{"platform":"windows","os":"windows-latest","exe":".exe","framework":"react","profile":"skipped"}]}'
assert_case cross-platform true "$windows_react" CROSS_PLATFORM=true
assert_case macos-contract-change true "$windows_react" MACOS=true
assert_case windows-all true "$windows_both" WINDOWS_ALL=true
assert_case shared-frontend true "$windows_both" FRONTEND_SHARED=true
assert_case vue true "$windows_vue" VUE=true
assert_case full true "$windows_both" FULL=true

echo "automation matrix cases passed"
