#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

usage() {
  cat <<'EOF'
Release LingXia npm packages.

Usage:
  scripts/release/npm.sh [--package bridge|elements|react|vue|html|page-runtime|polyfills|types|skill|all] [--publish] [--dry-run]

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
  types) targets=("types") ;;
  skill) targets=("skill") ;;
  all) targets=("bridge" "polyfills" "elements" "page-runtime" "html" "react" "vue" "types" "skill") ;;
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
    types) echo "$ROOT_DIR/packages/lingxia-types" ;;
    skill) echo "$ROOT_DIR/packages/lingxia-skill" ;;
    *) return 1 ;;
  esac
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

// Internal deps must be caret ranges on the same major.minor line as the
// publishing package: major.minor stays in lockstep across @lingxia/*,
// patch versions may drift per package.
const [major, minor] = pkg.version.split(".");
const expected = new RegExp(`^\\^${major}\\.${minor}\\.\\d+$`);

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
  console.error(`ERROR: ${pkg.name}@${pkg.version} expects internal @lingxia dependencies as caret ranges on the ${major}.${minor}.x line:`);
  for (const item of mismatches) {
    console.error(`  - ${item}`);
  }
  console.error("Run `scripts/release/version.sh <version>` to resync package versions before publishing.");
  process.exit(1);
}
NODE
}

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
