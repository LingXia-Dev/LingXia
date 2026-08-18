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

# A workspace crate that links into either binary but is not fingerprinted would
# let a change to it reuse a stale CLI. cargo answers this with features and
# target gates applied; without cargo the case is skipped (a fingerprint has to
# stay computable for any git ref, so the list itself stays hand-written).
if command -v cargo >/dev/null 2>&1 && command -v python3 >/dev/null 2>&1; then
  linked=$(cargo metadata --format-version 1 | python3 -c '
import json, os, sys

metadata = json.load(sys.stdin)
root = metadata["workspace_root"]
packages = {package["id"]: package for package in metadata["packages"]}
by_name = {package["name"]: package["id"] for package in metadata["packages"]}
nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}

seen = set()
queue = [by_name["lingxia-cli"], by_name["lingxia-devtools-cli"]]
while queue:
    package_id = queue.pop()
    if package_id in seen:
        continue
    seen.add(package_id)
    queue.extend(dependency["pkg"] for dependency in nodes[package_id]["deps"])

for package_id in seen:
    directory = os.path.dirname(packages[package_id]["manifest_path"])
    if directory.startswith(root):
        # git and the fingerprint list both speak forward slashes.
        print(os.path.relpath(directory, root).replace(os.sep, "/"))
' | sort)
  # `comm` needs both sides in the same collation, which differs between the
  # runners; match by line instead so the check cannot depend on sort order.
  # `grep -v` exits 1 when nothing is missing, which is the passing case.
  missing=$(
    grep -Fxv -f <(bash "$script_dir/cli-fingerprint.sh" --paths | tr -d '\r') \
      <<<"$(tr -d '\r' <<<"$linked")" || true
  )
  [[ -z "$missing" ]] || fail "linked crates absent from the fingerprint: $(tr '\n' ' ' <<<"$missing")"
fi

echo "cli fingerprint cases passed"
