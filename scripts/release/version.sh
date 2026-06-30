#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(realpath "$SCRIPT_DIR/../..")"
WORKSPACE_CARGO_TOML="$ROOT_DIR/Cargo.toml"
CLI_CARGO_TOML="$ROOT_DIR/tools/lingxia-cli/Cargo.toml"
# Showcase native host crate. Its `lingxia`/`lingxia_devtool` deps pin a version
# that the local path crates must satisfy, so a missed bump here breaks every
# example build (the patch is dropped and cargo falls back to the registry).
EXAMPLE_HOST_CARGO_TOML="$ROOT_DIR/examples/lingxia-showcase/native/Cargo.toml"

usage() {
  cat <<'EOF'
Update LingXia release versions in one step.

Usage:
  scripts/release/version.sh <version> [--component all|cli|npm:<package>] [--dry-run]

Arguments:
  <version>       Semver to apply (for example: 0.5.0)

Options:
  --component     Version scope. `all` bumps the base runtime in lockstep and
                  re-versions only the framework/tool packages that changed since
                  their last release tag (unchanged ones keep their version);
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
  - LingXia Runner crate versions + macOS Info.plist (tracks the CLI version)
  - example native host LingXia crate dependency versions
  - example native host Cargo.lock LingXia package versions
  - package versions under packages/*
  - the @lingxia/skill manifest version (kept in lockstep with its package.json)
  - internal @lingxia/* package dependency versions in published package.json files

On a patch bump, unchanged framework/tool npm packages keep their version (they
are not republished). On a minor or major bump, every framework/tool package is
moved to the new version too, so the release line advances in lockstep.

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

# The CLI keeps its own version line. Its major.minor mirrors the workspace
# (a CLI release is "the CLI for base runtime X.Y"), but its patch advances
# independently so CLI-only hotfixes never force a base bump and a base release
# never regresses the CLI. compute_cli_target picks the next CLI [package]
# version for `--component all`:
#   - same major.minor as the workspace target -> current CLI patch + 1
#   - different major.minor (new base line)     -> X.Y.0
compute_cli_target() {
  python3 - "$CLI_CARGO_TOML" "$1" <<'PY'
import re, sys
from pathlib import Path

text = Path(sys.argv[1]).read_text()
ws = sys.argv[2]
m = re.search(
    r'^\[package\]\n(?:(?!^\[).*\n)*?^version\s*=\s*"([^"]+)"',
    text,
    re.MULTILINE,
)
cur = m.group(1) if m else "0.0.0"

def parts(v):
    p = (v.split(".") + ["0", "0", "0"])[:3]
    return p[0], p[1], p[2]

wm, wn, _ = parts(ws)
cm, cn, cp = parts(cur)
if (wm, wn) == (cm, cn):
    try:
        patch = str(int(cp) + 1)
    except ValueError:
        patch = cp
    print(f"{wm}.{wn}.{patch}")
else:
    print(f"{wm}.{wn}.0")
PY
}

# update_cli_cargo [cli_package_version]
#   - cli_package_version: version to write to the CLI [package] (defaults to
#     $VERSION for `--component cli`; the `all` branch passes compute_cli_target).
#   - embedded component metadata (bridge/polyfills/types/crate/sdk/...) always
#     tracks the workspace version ($VERSION) and is only rewritten on `all`.
update_cli_cargo() {
  local cli_version="${1:-$VERSION}"
  python3 - "$CLI_CARGO_TOML" "$VERSION" "$DRY_RUN" "$COMPONENT" "$cli_version" <<'PY'
from pathlib import Path
import re
import sys

path = Path(sys.argv[1])
version = sys.argv[2]
dry_run = sys.argv[3] == "1"
component = sys.argv[4]
cli_version = sys.argv[5]
text = path.read_text()

package_re = re.compile(
    r'(^\[package\]\n(?:(?!^\[).*\n)*?)^version(?:\.workspace)?\s*=\s*(?:true|"[^"]+")',
    re.MULTILINE,
)
text, package_count = package_re.subn(rf'\g<1>version = "{cli_version}"', text, count=1)
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
        "browser-shell-webui-version",
        "resource-bundle-version",
    ]:
        pattern = rf'(^\s*{re.escape(key)}\s*=\s*")[^"]+(")'
        text, changed = re.subn(pattern, rf'\g<1>{version}\2', text, count=1, flags=re.MULTILINE)
        if changed != 1:
            raise SystemExit(f"failed to update package.metadata.lingxia.{key} in {path}")
        metadata_count += changed

if dry_run:
    print(f"would update {path}")
    print(f"  lingxia-cli package version -> {cli_version}")
    if component == "all":
        print(f"  embedded component versions -> {version} ({metadata_count} keys)")
else:
    path.write_text(text)
    print(f"updated {path}")
    print(f"  lingxia-cli package version -> {cli_version}")
    if component == "all":
        print(f"  embedded component versions -> {version} ({metadata_count} keys)")
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

  # The skill manifest carries its own version; keep it in lockstep whenever the
  # skill package itself is (re)versioned.
  case "$package_json" in
    */packages/lingxia-skill/package.json) sync_skill_manifest "$VERSION" ;;
  esac
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

# The LingXia Runner ships alongside the CLI: runner.sh names the release zip by
# the CLI version, and the runner crate manifests carry "keep in sync with
# tools/lingxia-cli" comments. So the runner tracks the CLI [package] version
# (not the workspace version). This bumps all three runner crates, the workspace
# dependency entry that points at the runner config crate (which update_workspace
# _cargo skips because its path is under tools/, not crates/), and the macOS
# app's Info.plist CFBundleShortVersionString.
update_runner_version() {
  local v="$1"
  local plist="$ROOT_DIR/tools/lingxia-runner/macos/Info.plist"
  local tomls=(
    "$ROOT_DIR/tools/lingxia-runner/macos/native/Cargo.toml"
    "$ROOT_DIR/tools/lingxia-runner/config/Cargo.toml"
    "$ROOT_DIR/tools/lingxia-runner/windows/Cargo.toml"
  )
  python3 - "$v" "$DRY_RUN" "$WORKSPACE_CARGO_TOML" "$plist" "${tomls[@]}" <<'PY'
import re, sys
from pathlib import Path

version = sys.argv[1]
dry = sys.argv[2] == "1"
workspace = Path(sys.argv[3])
plist = Path(sys.argv[4])
tomls = [Path(p) for p in sys.argv[5:]]

pkg_re = re.compile(r'(^\[package\]\n(?:(?!^\[).*\n)*?^version\s*=\s*")[^"]+(")', re.MULTILINE)
for t in tomls:
    text = t.read_text()
    new, n = pkg_re.subn(rf'\g<1>{version}\2', text, count=1)
    if n != 1:
        raise SystemExit(f"failed to set [package] version in {t}")
    if dry:
        print(f"would update {t} -> {version}")
    else:
        t.write_text(new)
        print(f"updated {t} -> {version}")

wtext = workspace.read_text()
wnew, wn = re.subn(
    r'(^lingxia-runner-config\s*=\s*\{[^}\n]*version\s*=\s*")[^"]+(")',
    rf'\g<1>{version}\2', wtext, count=1, flags=re.MULTILINE)
if wn == 1 and not dry:
    workspace.write_text(wnew)
print(f"{'would update' if dry else 'updated'} workspace dep lingxia-runner-config -> {version}" if wn == 1
      else "warning: lingxia-runner-config workspace dep not found")

ptext = plist.read_text()
pnew, pn = re.subn(
    r'(<key>CFBundleShortVersionString</key>\s*\n\s*<string>)[^<]*(</string>)',
    rf'\g<1>{version}\2', ptext, count=1)
if pn == 1 and not dry:
    plist.write_text(pnew)
print(f"{'would update' if dry else 'updated'} {plist} CFBundleShortVersionString -> {version}" if pn == 1
      else f"warning: CFBundleShortVersionString not found in {plist}")
PY
}

# The @lingxia/skill package ships a separate skill manifest with its own version
# field; keep it in lockstep with packages/lingxia-skill/package.json so the two
# never diverge. Called from update_package_json with the version being written,
# so it stays accurate under --dry-run. Targeted rewrite avoids reformatting.
sync_skill_manifest() {
  local v="$1"
  local manifest="$ROOT_DIR/packages/lingxia-skill/skill/skill-manifest.json"
  [[ -f "$manifest" ]] || return 0
  python3 - "$manifest" "$v" "$DRY_RUN" <<'PY'
import re, sys
from pathlib import Path

path = Path(sys.argv[1])
version = sys.argv[2]
dry = sys.argv[3] == "1"
text = path.read_text()
new, n = re.subn(r'("version"\s*:\s*")[^"]+(")', rf'\g<1>{version}\2', text, count=1)
if n != 1:
    raise SystemExit(f"failed to set version in {path}")
if dry:
    print(f"would update {path} -> {version}")
else:
    path.write_text(new)
    print(f"updated {path} -> {version}")
PY
}

# npm packages locked to the workspace version (base runtime). These always
# bump with --component all. Every other packages/* entry is framework/tools and
# only bumps when its source changed since its last release tag — an unchanged
# framework package keeps its version, so npm.sh sees it already published and
# skips it instead of republishing identical content under a new number.
BASE_NPM_PACKAGES="bridge polyfills types"

npm_short_name() { # packages/lingxia-foo/package.json -> foo
  local dir
  dir="$(basename "$(dirname "$1")")"
  printf '%s' "${dir#lingxia-}"
}

is_base_npm() {
  case " $BASE_NPM_PACKAGES " in
    *" $1 "*) return 0 ;;
  esac
  return 1
}

# 0 = bump (changed, or no prior release tag, or git unavailable — safe default);
# 1 = skip (unchanged since the last lingxia-<pkg>-v* release tag).
npm_pkg_changed() {
  local short="$1"
  local dir="$ROOT_DIR/packages/lingxia-$short"
  local tag
  tag="$(git -C "$ROOT_DIR" tag --list "lingxia-$short-v*" --sort=-v:refname 2>/dev/null | head -n1)"
  [[ -z "$tag" ]] && return 0
  git -C "$ROOT_DIR" diff --quiet "$tag" -- "$dir" 2>/dev/null && return 1
  return 0
}

if [[ "$COMPONENT" == "all" ]]; then
  update_workspace_cargo
  cli_version="$(compute_cli_target "$VERSION")"
  update_cli_cargo "$cli_version"
  update_runner_version "$cli_version"
  update_example_host_cargo
  update_example_host_lock

  # Minor/major bump (0.x -> 0.(x+1), or X -> X+1): force EVERY framework/tool
  # npm package to the new version too, so the line moves in lockstep and no
  # package is left behind with unsatisfiable ^old caret deps. Patch bump: keep
  # skip-unchanged (don't republish identical content under a new patch number).
  target_mm="${VERSION%.*}"
  while IFS= read -r package_json; do
    short="$(npm_short_name "$package_json")"
    cur="$(node -p "require('$package_json').version" 2>/dev/null || echo 0.0.0)"
    if is_base_npm "$short" || [[ "${cur%.*}" != "$target_mm" ]] || npm_pkg_changed "$short"; then
      update_package_json "$package_json"
    else
      echo "↳ skip lingxia-$short: unchanged patch-level package (stays at $cur)"
    fi
  done < <(find "$ROOT_DIR/packages" -mindepth 2 -maxdepth 2 -name package.json | sort)
elif [[ -n "$NPM_PACKAGE" ]]; then
  package_json="$ROOT_DIR/packages/lingxia-$NPM_PACKAGE/package.json"
  [[ -f "$package_json" ]] || { echo "Missing $package_json" >&2; exit 1; }
  update_package_json "$package_json" 0
else
  update_cli_cargo
  update_runner_version "$VERSION"
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
