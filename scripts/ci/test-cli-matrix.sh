#!/usr/bin/env bash

set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)

linux_x64='{"platform":"linux-x64","os":"ubuntu-latest","target":"x86_64-unknown-linux-gnu","exe":""}'
linux_arm64='{"platform":"linux-arm64","os":"ubuntu-24.04-arm","target":"aarch64-unknown-linux-gnu","exe":""}'
darwin_arm64='{"platform":"darwin-arm64","os":"macos-latest","target":"aarch64-apple-darwin","exe":""}'
darwin_x64='{"platform":"darwin-x64","os":"macos-latest","target":"x86_64-apple-darwin","exe":""}'
windows_x64='{"platform":"windows-x64","os":"windows-latest","target":"x86_64-pc-windows-msvc","exe":".exe"}'

assert_case() {
  local name="$1"
  local expected_build="$2"
  local expected_matrix="$3"
  shift 3

  local output
  output=$(env RUST=false CROSS=false "$@" bash "$script_dir/cli-matrix.sh")

  local actual_build
  actual_build=$(sed -n 's/^cli_build=//p' <<<"$output")
  local actual_matrix
  actual_matrix=$(sed -n 's/^cli_matrix=//p' <<<"$output")

  if [[ "$actual_build" != "$expected_build" || "$actual_matrix" != "$expected_matrix" ]]; then
    echo "cli matrix case failed: $name" >&2
    echo "expected cli_build=$expected_build matrix=$expected_matrix" >&2
    echo "actual   cli_build=$actual_build matrix=$actual_matrix" >&2
    return 1
  fi
}

assert_case skipped false "{\"include\":[$linux_x64]}"
assert_case "rust but cli inputs unchanged" false "{\"include\":[$linux_x64]}" \
  RUST=true CLI_INPUTS_CHANGED=false
assert_case pr true \
  "{\"include\":[$linux_x64,$linux_arm64,$darwin_arm64,$windows_x64]}" \
  RUST=true
assert_case main true \
  "{\"include\":[$linux_x64,$linux_arm64,$darwin_arm64,$darwin_x64,$windows_x64]}" \
  RUST=true CROSS=true

echo "cli matrix cases passed"
