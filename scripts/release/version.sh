#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(realpath "$SCRIPT_DIR/../..")"
WORKSPACE_CARGO_TOML="$ROOT_DIR/Cargo.toml"
EXAMPLE_HOST_CARGO_TOML="$ROOT_DIR/examples/lingxia-lib/Cargo.toml"

usage() {
  cat <<'EOF'
Update LingXia release versions in one step.

Usage:
  scripts/release/version.sh <version> [--dry-run]

Arguments:
  <version>       Semver to apply (for example: 0.5.0)

Options:
  --dry-run       Print the files that would change without modifying them.
  -h, --help      Show help.

This updates:
  - workspace.package.version in Cargo.toml
  - workspace crate dependency versions in Cargo.toml
  - example native host LingXia crate dependency versions
  - package versions under packages/*
  - internal @lingxia/* package dependency versions in published package.json files
EOF
}

DRY_RUN=0
VERSION=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) DRY_RUN=1 ;;
    -h|--help) usage; exit 0 ;;
    *)
      if [[ -n "$VERSION" ]]; then
        echo "Unexpected argument: $1" >&2
        usage
        exit 2
      fi
      VERSION="$1"
      ;;
  esac
  shift
done

if [[ -z "$VERSION" ]]; then
  usage
  exit 2
fi

if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "Invalid version: $VERSION (expected x.y.z)" >&2
  exit 2
fi

update_workspace_cargo() {
  python3 - "$WORKSPACE_CARGO_TOML" "$VERSION" "$DRY_RUN" <<'PY'
from pathlib import Path
import re
import sys

path = Path(sys.argv[1])
version = sys.argv[2]
dry_run = sys.argv[3] == "1"
text = path.read_text()

workspace_package_re = re.compile(
    r'(^\[workspace\.package\]\n(?:.*\n)*?^version\s*=\s*")[^"]+(")',
    re.MULTILINE,
)
text, pkg_count = workspace_package_re.subn(rf"\g<1>{version}\2", text, count=1)

in_ws_deps = False
out_lines = []
deps_count = 0
for line in text.splitlines(True):
    stripped = line.strip()
    if stripped == "[workspace.dependencies]":
        in_ws_deps = True
        out_lines.append(line)
        continue
    if in_ws_deps and stripped.startswith("[") and stripped != "[workspace.dependencies]":
        in_ws_deps = False

    if (
        in_ws_deps
        and 'path = "crates/' in line
        and 'version = "' in line
        # Match `lingxia`, `lingxia-foo`, `lingxia_foo` (underscore key for
        # the few crates whose Rust ident requires it, e.g. lingxia_devtool
        # which keys to the dash-named package), and the bare `lxapp` alias.
        and re.match(r'^(lingxia(?:[_-][a-z0-9_-]+)?|lxapp)\s*=', line)
    ):
        new_line, n = re.subn(r'version\s*=\s*"[^"]+"', f'version = "{version}"', line, count=1)
        line = new_line
        deps_count += n
    out_lines.append(line)

new_text = "".join(out_lines)

if dry_run:
    print(f"would update {path}")
    print(f"  workspace.package.version -> {version}")
    print(f"  workspace dependency versions updated: {deps_count}")
else:
    path.write_text(new_text)
    print(f"updated {path}")
    print(f"  workspace.package.version -> {version}")
    print(f"  workspace dependency versions updated: {deps_count}")
PY
}

update_package_json() {
  local package_json="$1"
  node - "$package_json" "$VERSION" "$DRY_RUN" <<'NODE'
const fs = require("fs");
const path = process.argv[2];
const version = process.argv[3];
const dryRun = process.argv[4] === "1";

const pkg = JSON.parse(fs.readFileSync(path, "utf8"));
const sections = ["dependencies", "peerDependencies", "optionalDependencies", "devDependencies"];

pkg.version = version;

function rewriteLingxiaRange(spec, version) {
  if (typeof spec !== "string") return spec;
  if (spec.startsWith("file:")) return spec;

  const match = spec.match(/^(\^|~|>=|<=|>|<|=)?\s*\d+\.\d+\.\d+$/);
  if (match) {
    const prefix = match[1] ?? "";
    return `${prefix}${version}`;
  }

  return spec;
}

for (const section of sections) {
  const deps = pkg[section];
  if (!deps) continue;
  for (const [name, value] of Object.entries(deps)) {
    if (!name.startsWith("@lingxia/")) continue;
    deps[name] = rewriteLingxiaRange(value, version);
  }
}

if (dryRun) {
  console.log(`would update ${path}`);
} else {
  fs.writeFileSync(path, JSON.stringify(pkg, null, 2) + "\n");
  console.log(`updated ${path}`);
}
NODE
}

update_example_host_cargo() {
  [[ -f "$EXAMPLE_HOST_CARGO_TOML" ]] || return 0

  python3 - "$EXAMPLE_HOST_CARGO_TOML" "$VERSION" "$DRY_RUN" <<'PY'
from pathlib import Path
import re
import sys

path = Path(sys.argv[1])
version = sys.argv[2]
dry_run = sys.argv[3] == "1"
text = path.read_text()

patterns = [
    r'(^lingxia\s*=\s*\{[^}\n]*version\s*=\s*")[^"]+(")',
    r'(^lingxia_devtool\s*=\s*\{[^}\n]*version\s*=\s*")[^"]+(")',
]

count = 0
for pattern in patterns:
    text, changed = re.subn(pattern, rf"\g<1>{version}\2", text, count=1, flags=re.MULTILINE)
    count += changed

if dry_run:
    print(f"would update {path}")
    print(f"  example host dependency versions updated: {count}")
else:
    path.write_text(text)
    print(f"updated {path}")
    print(f"  example host dependency versions updated: {count}")
PY
}

update_workspace_cargo
update_example_host_cargo

while IFS= read -r package_json; do
  update_package_json "$package_json"
done < <(find "$ROOT_DIR/packages" -mindepth 2 -maxdepth 2 -name package.json | sort)

if [[ "$DRY_RUN" -eq 1 ]]; then
  echo ""
  echo "Dry run complete."
else
  echo ""
  echo "✅ Version updated to $VERSION"
fi
