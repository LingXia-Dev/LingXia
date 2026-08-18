#!/usr/bin/env bash
# Fingerprint of inputs baked into the `lingxia` / `lxdev` binaries.
#
# The CLI orchestrates a cargo build of the *current* workspace host; it does
# not embed windows-sdk / showcase / react. Those paths must not be in this
# list or every platform PR would rebuild the CLI.

set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd "$script_dir/../.." && pwd)

# First-party sources compiled into the two bins, plus the lockfile so a
# third-party bump that relinks them is visible.
CLI_FINGERPRINT_PATHS=(
  tools/lingxia-cli
  tools/lingxia-devtools-cli
  packages/lingxia-bridge
  packages/lingxia-polyfills
  crates/lingxia-control-protocol
  crates/lingxia-control-commands
  crates/lingxia-app-context
  Cargo.lock
)

if [[ "${1:-}" == "--paths" ]]; then
  printf '%s\n' "${CLI_FINGERPRINT_PATHS[@]}"
  exit 0
fi

ref=${1:-HEAD}

# `git ls-tree -r` is empty for a missing path at that ref (old tags).
# Hash the listing so two refs with the same trees produce the same id.
(
  cd "$repo_root"
  for path in "${CLI_FINGERPRINT_PATHS[@]}"; do
    echo "$path"
    git ls-tree -r "$ref" -- "$path"
  done
) | git hash-object --stdin
