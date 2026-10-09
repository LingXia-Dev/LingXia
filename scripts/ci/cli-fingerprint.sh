#!/usr/bin/env bash
# Fingerprint of inputs baked into the `lingxia` / `lxdev` binaries.
#
# The CLI orchestrates a cargo build of the *current* workspace host; it does
# not link windows-sdk / showcase / react implementations. Only the SDK
# manifest used by scaffolding belongs here, not the whole platform tree.

set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd "$script_dir/../.." && pwd)

# First-party sources compiled into the two bins, plus Cargo.lock so a
# third-party bump that relinks them is visible.
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
)

# The npm lock spans every workspace package; only the entries the embedded
# bridge/polyfills build resolves to can change the CLI.
CLI_NPM_LOCK=packages/package-lock.json
CLI_NPM_WORKSPACES=(lingxia-bridge lingxia-polyfills)

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
  echo "$CLI_NPM_LOCK ${CLI_NPM_WORKSPACES[*]}"
  if git cat-file -e "$ref:$CLI_NPM_LOCK" 2>/dev/null; then
    git cat-file blob "$ref:$CLI_NPM_LOCK" |
      node "$script_dir/npm-lock-closure.mjs" "${CLI_NPM_WORKSPACES[@]}"
  fi
) | git hash-object --stdin
