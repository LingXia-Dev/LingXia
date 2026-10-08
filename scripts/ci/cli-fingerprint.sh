#!/usr/bin/env bash
# Fingerprint of inputs baked into the `lingxia` / `lxdev` binaries.
#
# The CLI orchestrates a cargo build of the *current* workspace host; it does
# not link windows-sdk / showcase / react implementations. Only the SDK
# manifest used by scaffolding belongs here, not the whole platform tree.

set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd "$script_dir/../.." && pwd)

# First-party sources compiled into the two bins, plus the lockfiles so a
# third-party bump that relinks them — or rebuilds the embedded JS — is visible.
# Kept in step with the workspace closure of the two bin crates by
# test-cli-fingerprint.sh, which fails when a linked crate is missing here.
CLI_FINGERPRINT_PATHS=(
  tools/lingxia-cli
  tools/lingxia-devtools-cli
  tools/lingxia-runner/devices.json
  crates/lingxia-windows-sdk/Cargo.toml
  design/icons/svg
  design/app-icon
  i18n/permission/cli
  crates/lingxia-app-context
  docs/skill
  crates/lingxia-control-commands
  crates/lingxia-control-protocol
  crates/lingxia-device-io
  crates/lingxia-log
  crates/lingxia-provider
  crates/lingxia-settings
  crates/lingxia-update
  packages/lingxia-bridge
  packages/lingxia-polyfills
  Cargo.lock
  Cargo.toml
  rust-toolchain.toml
  rust-toolchain
  packages/package.json
  packages/package-lock.json
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
