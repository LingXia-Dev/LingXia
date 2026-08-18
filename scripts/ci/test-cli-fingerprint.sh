#!/usr/bin/env bash

set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd "$script_dir/../.." && pwd)

fail() {
  echo "cli fingerprint case failed: $*" >&2
  exit 1
}

cd "$repo_root"

paths=$(bash "$script_dir/cli-fingerprint.sh" --paths)
echo "$paths" | grep -qx 'tools/lingxia-cli' || fail "paths must include tools/lingxia-cli"
echo "$paths" | grep -qx 'Cargo.lock' || fail "paths must include Cargo.lock"
if echo "$paths" | grep -qx 'examples/lingxia-showcase'; then
  fail "showcase must not be in the CLI fingerprint"
fi

head_fp=$(bash "$script_dir/cli-fingerprint.sh" HEAD)
again=$(bash "$script_dir/cli-fingerprint.sh" HEAD)
[[ "$head_fp" == "$again" ]] || fail "fingerprint must be stable"
[[ "$head_fp" =~ ^[0-9a-f]{40}$ ]] || fail "fingerprint must be a 40-char hex object id (got $head_fp)"

unchanged=$(bash "$script_dir/cli-inputs-changed.sh" HEAD HEAD)
[[ "$unchanged" == "false" ]] || fail "HEAD vs HEAD must be unchanged"

echo "cli fingerprint cases passed"
