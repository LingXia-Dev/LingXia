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
  actual_matrix=$(sed -n 's/^automation_matrix=//p' <<<"$output" | jq -S -c .)
  expected_matrix=$(jq -S -c . <<<"$expected_matrix")

  if [[ "$actual_automation" != "$expected_automation" || "$actual_matrix" != "$expected_matrix" ]]; then
    echo "matrix case failed: $name" >&2
    echo "expected automation=$expected_automation matrix=$expected_matrix" >&2
    echo "actual   automation=$actual_automation matrix=$actual_matrix" >&2
    return 1
  fi
}

assert_case none false \
  '{"include":[{"platform":"macos","os":"macos-latest","exe":"","frameworks":"react","profile":"skipped"}]}'
assert_case cross-platform true \
  '{"include":[{"platform":"macos","os":"macos-latest","exe":"","frameworks":"react","profile":"react"},{"platform":"windows","os":"windows-latest","exe":".exe","frameworks":"react","profile":"react"}]}' \
  CROSS_PLATFORM=true
assert_case macos true \
  '{"include":[{"platform":"macos","os":"macos-latest","exe":"","frameworks":"react","profile":"react"}]}' \
  MACOS=true
assert_case windows-all true \
  '{"include":[{"platform":"windows","os":"windows-latest","exe":".exe","frameworks":"react vue","profile":"react-vue"}]}' \
  WINDOWS_ALL=true
assert_case shared-frontend true \
  '{"include":[{"platform":"macos","os":"macos-latest","exe":"","frameworks":"react vue","profile":"react-vue"}]}' \
  FRONTEND_SHARED=true
assert_case vue true \
  '{"include":[{"platform":"macos","os":"macos-latest","exe":"","frameworks":"vue","profile":"vue"}]}' \
  VUE=true
assert_case full true \
  '{"include":[{"platform":"macos","os":"macos-latest","exe":"","frameworks":"react vue","profile":"react-vue"},{"platform":"windows","os":"windows-latest","exe":".exe","frameworks":"react vue","profile":"react-vue"}]}' \
  FULL=true

echo "automation matrix cases passed"
