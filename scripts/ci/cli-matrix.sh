#!/usr/bin/env bash

set -euo pipefail

is_true() {
  [[ "${1:-false}" == "true" ]]
}

linux_x64='{"platform":"linux-x64","os":"ubuntu-latest","target":"x86_64-unknown-linux-gnu","exe":""}'
linux_arm64='{"platform":"linux-arm64","os":"ubuntu-24.04-arm","target":"aarch64-unknown-linux-gnu","exe":""}'
darwin_arm64='{"platform":"darwin-arm64","os":"macos-latest","target":"aarch64-apple-darwin","exe":""}'
darwin_x64='{"platform":"darwin-x64","os":"macos-latest","target":"x86_64-apple-darwin","exe":""}'
windows_x64='{"platform":"windows-x64","os":"windows-latest","target":"x86_64-pc-windows-msvc","exe":".exe"}'

if ! is_true "${RUST:-false}"; then
  echo "cli_build=false"
  # GitHub expands the matrix before evaluating the job condition.
  echo "cli_matrix={\"include\":[$linux_x64]}"
  exit 0
fi

echo "cli_build=true"
if is_true "${CROSS:-false}"; then
  echo "cli_matrix={\"include\":[$linux_x64,$linux_arm64,$darwin_arm64,$darwin_x64,$windows_x64]}"
else
  # PR gates skip the Intel macOS cross-compile — it occupies a hosted macOS
  # slot for a `lingxia version` smoke. main + release still build it.
  echo "cli_matrix={\"include\":[$linux_x64,$linux_arm64,$darwin_arm64,$windows_x64]}"
fi
