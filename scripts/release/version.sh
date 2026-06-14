#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(realpath "$SCRIPT_DIR/../..")"
WORKSPACE_CARGO_TOML="$ROOT_DIR/Cargo.toml"
CLI_CARGO_TOML="$ROOT_DIR/tools/lingxia-cli/Cargo.toml"
EXAMPLE_HOST_CARGO_TOML="$ROOT_DIR/examples/lingxia-showcase/lingxia-lib/Cargo.toml"

usage() {
  cat <<'EOF'
Update LingXia release versions in one step.

Usage:
  scripts/release/version.sh <version> [--component all|cli|npm:<package>] [--dry-run]

Arguments:
  <version>       Semver to apply (for example: 0.5.0)

Options:
  --component     Version scope. `all` keeps the lockstep release bump;
                  `cli` bumps only tools/lingxia-cli; `npm:<package>` bumps a
                  single framework/tool npm package (elements|react|vue|html|
                  page-runtime|skill). bridge/polyfills/types are base-runtime
                  packages locked to the workspace — bump them via `all`.
  --dry-run       Print the files that would change without modifying them.
  -h, --help      Show help.

npm package tiers (see release notes):
  - base runtime  (bridge, polyfills, types): locked to the workspace version.
                  bridge & polyfills are embedded into the CLI as app assets
                  (build.rs enforces the match); ship via --component all.
  - framework     (page-runtime, elements, react, vue, html): major.minor
                  tracks the workspace; patch may drift via npm:<package>.
  - tools         (skill): independent; npm:skill.

With --component all (default), this updates:
  - workspace.package.version in Cargo.toml
  - workspace crate dependency versions in Cargo.toml
  - lingxia-cli package version and embedded LingXia component versions
  - example native host LingXia crate dependency versions
  - example native host Cargo.lock LingXia package versions
  - package versions under packages/*
  - internal @lingxia/* package dependency versions in published package.json files

With --component cli, this updates:
  - lingxia-cli package version only
  - root Cargo.lock package metadata when Cargo needs it

With --component npm:<package> (framework/tools only), this updates:
  - that package's package.json version only. Internal @lingxia/* dependency
    ranges are left untouched: they are caret ranges (^0.x.y), so a patch
    bump stays inside the range siblings already accept. Keep major.minor
    in lockstep via --component all; this escape hatch is for patch drift.
    bridge/polyfills/types are rejected here — release them via --component all.
EOF
}

DRY_RUN=0
COMPONENT="all"
VERSION=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) DRY_RUN=1 ;;
    --component)
      if [[ $# -lt 2 ]]; then
        echo "Missing value for --component" >&2
        exit 2
      fi
      COMPONENT="${2:-}"
      shift 2
      continue
      ;;
    --cli-only) COMPONENT="cli" ;;
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

NPM_PACKAGE=""
case "$COMPONENT" in
  all|cli) ;;
  npm:*)
    NPM_PACKAGE="${COMPONENT#npm:}"
    # Per-package bumps are only allowed for the framework tier and standalone
    # tools. bridge/polyfills/types are base-runtime packages locked to the
    # workspace version: bridge & polyfills are embedded into the CLI as app
    # assets (tools/lingxia-cli/build.rs panics on a version mismatch) and must
    # ship together with the rust workspace via `--component all`.
    case "$NPM_PACKAGE" in
      elements|react|vue|html|page-runtime|skill) ;;
      bridge|polyfills|types)
        echo "Refusing npm:$NPM_PACKAGE — base-runtime package locked to the workspace version; release it with '--component all'." >&2
        exit 2
        ;;
      *)
        echo "Invalid npm package: $NPM_PACKAGE (expected elements|react|vue|html|page-runtime|skill)" >&2
        exit 2
        ;;
    esac
    ;;
  *)
    echo "Invalid component: $COMPONENT (expected all, cli, or npm:<package>)" >&2
    exit 2
    ;;
esac

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

update_cli_cargo() {
  python3 - "$CLI_CARGO_TOML" "$VERSION" "$DRY_RUN" "$COMPONENT" <<'PY'
from pathlib import Path
import re
import sys

path = Path(sys.argv[1])
version = sys.argv[2]
dry_run = sys.argv[3] == "1"
component = sys.argv[4]
text = path.read_text()

package_re = re.compile(
    r'(^\[package\]\n(?:(?!^\[).*\n)*?)^version(?:\.workspace)?\s*=\s*(?:true|"[^"]+")',
    re.MULTILINE,
)
text, package_count = package_re.subn(rf'\g<1>version = "{version}"', text, count=1)
if package_count != 1:
    raise SystemExit(f"failed to update [package] version in {path}")

metadata_count = 0
if component == "all":
    for key in [
        "bridge-version",
        "polyfills-version",
        "types-version",
        "rust-crate-version",
        "sdk-version",
        "shell-webui-version",
        "resource-bundle-version",
    ]:
        pattern = rf'(^\s*{re.escape(key)}\s*=\s*")[^"]+(")'
        text, changed = re.subn(pattern, rf'\g<1>{version}\2', text, count=1, flags=re.MULTILINE)
        if changed != 1:
            raise SystemExit(f"failed to update package.metadata.lingxia.{key} in {path}")
        metadata_count += changed

if dry_run:
    print(f"would update {path}")
    print(f"  lingxia-cli package version -> {version}")
    if component == "all":
        print(f"  embedded component versions updated: {metadata_count}")
else:
    path.write_text(text)
    print(f"updated {path}")
    print(f"  lingxia-cli package version -> {version}")
    if component == "all":
        print(f"  embedded component versions updated: {metadata_count}")
PY
}

update_package_json() {
  local package_json="$1"
  local rewrite_deps="${2:-1}"
  node - "$package_json" "$VERSION" "$DRY_RUN" "$rewrite_deps" <<'NODE'
const fs = require("fs");
const path = process.argv[2];
const version = process.argv[3];
const dryRun = process.argv[4] === "1";
const rewriteDeps = process.argv[5] === "1";

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

if (rewriteDeps) {
  for (const section of sections) {
    const deps = pkg[section];
    if (!deps) continue;
    for (const [name, value] of Object.entries(deps)) {
      if (!name.startsWith("@lingxia/")) continue;
      deps[name] = rewriteLingxiaRange(value, version);
    }
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

update_example_host_lock() {
  [[ -f "$EXAMPLE_HOST_CARGO_TOML" ]] || return 0
  [[ -f "$(dirname "$EXAMPLE_HOST_CARGO_TOML")/Cargo.lock" ]] || return 0

  if [[ "$DRY_RUN" -eq 1 ]]; then
    echo "would update $(dirname "$EXAMPLE_HOST_CARGO_TOML")/Cargo.lock"
    echo "  example host lockfile LingXia packages -> $VERSION"
    return 0
  fi

  cargo update \
    --manifest-path "$EXAMPLE_HOST_CARGO_TOML" \
    -p lingxia \
    -p lingxia-devtool
}

update_root_lock() {
  [[ -f "$ROOT_DIR/Cargo.lock" ]] || return 0

  if [[ "$DRY_RUN" -eq 1 ]]; then
    echo "would refresh $ROOT_DIR/Cargo.lock if Cargo needs metadata changes"
    return 0
  fi

  cargo metadata \
    --manifest-path "$ROOT_DIR/Cargo.toml" \
    --format-version 1 \
    --no-deps \
    >/dev/null
}

if [[ "$COMPONENT" == "all" ]]; then
  update_workspace_cargo
  update_cli_cargo
  update_example_host_cargo
  update_example_host_lock

  while IFS= read -r package_json; do
    update_package_json "$package_json"
  done < <(find "$ROOT_DIR/packages" -mindepth 2 -maxdepth 2 -name package.json | sort)
elif [[ -n "$NPM_PACKAGE" ]]; then
  package_json="$ROOT_DIR/packages/lingxia-$NPM_PACKAGE/package.json"
  [[ -f "$package_json" ]] || { echo "Missing $package_json" >&2; exit 1; }
  update_package_json "$package_json" 0
else
  update_cli_cargo
fi

if [[ -z "$NPM_PACKAGE" ]]; then
  update_root_lock
fi

if [[ "$DRY_RUN" -eq 1 ]]; then
  echo ""
  echo "Dry run complete."
else
  echo ""
  echo "✅ Version updated to $VERSION ($COMPONENT)"
fi
