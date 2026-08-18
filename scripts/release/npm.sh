#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

usage() {
  cat <<'EOF'
Release LingXia npm packages.

Usage:
  scripts/release/npm.sh [--package bridge|elements|react|vue|html|page-runtime|polyfills|terminal-settings|browser-shell-webui|types|skill|all] [--publish] [--dry-run]

Options:
  --package <name>  Package set to process (default: all)
  --publish         Publish to npm registry.
  --dry-run         Build + npm pack --dry-run.
  -h, --help        Show help.
EOF
}

if [[ $# -eq 0 ]]; then
  usage
  exit 2
fi

PACKAGE_SET="all"
PUBLISH=0
DRY_RUN=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --package) PACKAGE_SET="${2:-}"; shift ;;
    --publish) PUBLISH=1 ;;
    --dry-run) DRY_RUN=1 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown option: $1" >&2; usage; exit 2 ;;
  esac
  shift
done

if [[ "$PUBLISH" -eq 0 && "$DRY_RUN" -eq 0 ]]; then
  DRY_RUN=1
fi

case "$PACKAGE_SET" in
  bridge) targets=("bridge") ;;
  elements) targets=("elements") ;;
  react) targets=("react") ;;
  vue) targets=("vue") ;;
  html) targets=("html") ;;
  page-runtime) targets=("page-runtime") ;;
  polyfills) targets=("polyfills") ;;
  terminal-settings) targets=("terminal-settings") ;;
  browser-shell-webui) targets=("browser-shell-webui") ;;
  types) targets=("types") ;;
  skill) targets=("skill") ;;
  # Tier 1, then framework in dep order (bridge → elements/page-runtime → html/react/vue),
  # then prebuilt lxapps and the standalone skill.
  all) targets=("bridge" "polyfills" "types" "elements" "page-runtime" "html" "react" "vue" "terminal-settings" "browser-shell-webui" "skill") ;;
  *) echo "Unknown package set: $PACKAGE_SET" >&2; exit 2 ;;
esac

pkg_dir() {
  case "$1" in
    bridge) echo "$ROOT_DIR/packages/lingxia-bridge" ;;
    elements) echo "$ROOT_DIR/packages/lingxia-elements" ;;
    react) echo "$ROOT_DIR/packages/lingxia-react" ;;
    vue) echo "$ROOT_DIR/packages/lingxia-vue" ;;
    html) echo "$ROOT_DIR/packages/lingxia-html" ;;
    page-runtime) echo "$ROOT_DIR/packages/lingxia-page-runtime" ;;
    polyfills) echo "$ROOT_DIR/packages/lingxia-polyfills" ;;
    terminal-settings) echo "$ROOT_DIR/packages/lingxia-terminal-settings" ;;
    browser-shell-webui) echo "$ROOT_DIR/crates/lingxia-browser-shell/webui" ;;
    types) echo "$ROOT_DIR/packages/lingxia-types" ;;
    skill) echo "$ROOT_DIR/packages/lingxia-skill" ;;
    *) return 1 ;;
  esac
}

verify_package_inventory() {
  local missing=()
  local dir name
  for dir in "$ROOT_DIR"/packages/lingxia-*/; do
    [[ -f "${dir}package.json" ]] || continue
    name="$(node -p "require('${dir}package.json').name")"
    if ! pkg_dir_for_name "$name" >/dev/null; then
      missing+=("$name")
    fi
  done
  name="$(node -p "require('$ROOT_DIR/crates/lingxia-browser-shell/webui/package.json').name")"
  if ! pkg_dir_for_name "$name" >/dev/null; then
    missing+=("$name")
  fi
  if [[ "${#missing[@]}" -gt 0 ]]; then
    echo "Workspace package(s) missing from the npm release inventory: ${missing[*]}" >&2
    echo "Add each package to pkg_dir / the --package all list." >&2
    exit 1
  fi
}

pkg_dir_for_name() {
  case "$1" in
    @lingxia/bridge) pkg_dir bridge ;;
    @lingxia/polyfills) pkg_dir polyfills ;;
    @lingxia/types) pkg_dir types ;;
    @lingxia/elements) pkg_dir elements ;;
    @lingxia/page-runtime) pkg_dir page-runtime ;;
    @lingxia/html) pkg_dir html ;;
    @lingxia/react) pkg_dir react ;;
    @lingxia/vue) pkg_dir vue ;;
    @lingxia/terminal-settings) pkg_dir terminal-settings ;;
    @lingxia/browser-shell-webui) pkg_dir browser-shell-webui ;;
    @lingxia/skill) pkg_dir skill ;;
    *) return 1 ;;
  esac
}

verify_publish_order() {
  node - "$ROOT_DIR" "${targets[@]}" <<'NODE'
const fs = require("fs");
const path = require("path");
const root = process.argv[2];
const targets = process.argv.slice(3);
const dirOf = {
  bridge: "packages/lingxia-bridge",
  polyfills: "packages/lingxia-polyfills",
  types: "packages/lingxia-types",
  elements: "packages/lingxia-elements",
  "page-runtime": "packages/lingxia-page-runtime",
  html: "packages/lingxia-html",
  react: "packages/lingxia-react",
  vue: "packages/lingxia-vue",
  "terminal-settings": "packages/lingxia-terminal-settings",
  "browser-shell-webui": "crates/lingxia-browser-shell/webui",
  skill: "packages/lingxia-skill",
};
const nameOf = {};
for (const target of targets) {
  const pkg = JSON.parse(fs.readFileSync(path.join(root, dirOf[target], "package.json"), "utf8"));
  nameOf[pkg.name] = target;
}
const index = Object.fromEntries(targets.map((t, i) => [t, i]));
const problems = [];
for (const target of targets) {
  const pkg = JSON.parse(fs.readFileSync(path.join(root, dirOf[target], "package.json"), "utf8"));
  for (const section of ["dependencies", "peerDependencies", "optionalDependencies"]) {
    const deps = pkg[section];
    if (!deps) continue;
    for (const [dep, spec] of Object.entries(deps)) {
      if (!dep.startsWith("@lingxia/") || typeof spec !== "string" || spec.startsWith("file:")) {
        continue;
      }
      const depTarget = nameOf[dep];
      if (depTarget === undefined) {
        problems.push(`${target} ${section} lists ${dep}, which is not in this release set`);
      } else if (index[depTarget] > index[target]) {
        problems.push(`${target} depends on ${depTarget}, published later`);
      }
    }
  }
}
if (problems.length > 0) {
  console.error("npm publish order is not a valid dependency order:");
  for (const problem of problems) console.error(`  - ${problem}`);
  process.exit(1);
}
NODE
}

npm_package_published() {
  local name="$1"
  local version="$2"
  npm view "$name@$version" version >/dev/null 2>&1
}

verify_internal_lingxia_versions() {
  local dir="$1"
  node - "$dir" <<'NODE'
const fs = require("fs");
const path = process.argv[2];
const pkg = JSON.parse(fs.readFileSync(`${path}/package.json`, "utf8"));
const sections = ["dependencies", "peerDependencies", "optionalDependencies"];
const mismatches = [];

// Internal deps must be tilde ranges on the same major.minor line as the
// publishing package: major.minor stays in lockstep across @lingxia/*,
// patch versions may drift per package.
const [major, minor] = pkg.version.split(".");
const expected = new RegExp(`^~${major}\\.${minor}\\.\\d+$`);

for (const section of sections) {
  const deps = pkg[section];
  if (!deps) continue;
  for (const [name, spec] of Object.entries(deps)) {
    if (!name.startsWith("@lingxia/")) continue;
    if (name === pkg.name) continue;
    if (!expected.test(spec)) {
      mismatches.push(`${section}.${name}=${spec}`);
    }
  }
}

if (mismatches.length > 0) {
  console.error(`ERROR: ${pkg.name}@${pkg.version} expects internal @lingxia dependencies as tilde ranges on the ${major}.${minor}.x line:`);
  for (const item of mismatches) {
    console.error(`  - ${item}`);
  }
  console.error("Run `scripts/release/version.sh <version>` to resync package versions before publishing.");
  process.exit(1);
}
NODE
}

verify_package_inventory
if [[ "$PACKAGE_SET" == "all" ]]; then
  verify_publish_order
fi

for target in "${targets[@]}"; do
  dir="$(pkg_dir "$target")"
  name="$(node -p "require('$dir/package.json').name")"
  version="$(node -p "require('$dir/package.json').version")"

  echo ""
  echo "=========================================="
  echo "Processing $name@$version ($target)"
  echo "=========================================="

  if [[ "$PUBLISH" -eq 1 && "$DRY_RUN" -eq 0 ]] && npm_package_published "$name" "$version"; then
    echo "✓ $name@$version already published, skipping"
    continue
  fi

  verify_internal_lingxia_versions "$dir"

  if [[ -f "$dir/package-lock.json" ]]; then
    (cd "$dir" && npm ci)
  else
    (cd "$dir" && npm install)
  fi

  if node -e "const p=require('$dir/package.json'); process.exit(p.scripts && p.scripts.build ? 0 : 1)" >/dev/null 2>&1; then
    (cd "$dir" && npm run build)
  fi

  if [[ "$DRY_RUN" -eq 1 ]]; then
    (cd "$dir" && npm pack --dry-run)
    continue
  fi

  (cd "$dir" && npm publish --access public)
  echo "✓ Published $name@$version"
done

echo ""
echo "✅ npm release script completed."
