#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

usage() {
  cat <<'EOF'
Release LingXia npm packages.

Usage:
  scripts/release/npm.sh [--package bridge|elements|react|vue|html|page-runtime|polyfills|terminal-settings|types|test|all] [--publish] [--dry-run]

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
VERIFY_ONLY=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --package) PACKAGE_SET="${2:-}"; shift ;;
    --publish) PUBLISH=1 ;;
    --dry-run) DRY_RUN=1 ;;
    --verify-inventory) VERIFY_ONLY=1 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown option: $1" >&2; usage; exit 2 ;;
  esac
  shift
done

if [[ "$PUBLISH" -eq 0 && "$DRY_RUN" -eq 0 ]]; then
  DRY_RUN=1
fi

# Tier 1, then framework in dep order (bridge → elements/page-runtime → html/react/vue),
# then prebuilt lxapps.
ALL_TARGETS=("bridge" "polyfills" "types" "test" "elements" "page-runtime" "html" "react" "vue" "terminal-settings")

case "$PACKAGE_SET" in
  bridge) targets=("bridge") ;;
  elements) targets=("elements") ;;
  react) targets=("react") ;;
  vue) targets=("vue") ;;
  html) targets=("html") ;;
  page-runtime) targets=("page-runtime") ;;
  polyfills) targets=("polyfills") ;;
  terminal-settings) targets=("terminal-settings") ;;
  types) targets=("types") ;;
  test) targets=("test") ;;
  all) targets=("${ALL_TARGETS[@]}") ;;
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
    types) echo "$ROOT_DIR/packages/lingxia-types" ;;
    test) echo "$ROOT_DIR/packages/lingxia-test" ;;
    *) return 1 ;;
  esac
}

verify_package_inventory() {
  local unmapped=() unreleased=()
  local dir name target
  for dir in "$ROOT_DIR"/packages/lingxia-*/; do
    [[ -f "${dir}package.json" ]] || continue
    name="$(node -p "require('${dir}package.json').name")"
    if ! target="$(pkg_target_for_name "$name")"; then
      unmapped+=("$name")
    elif ! printf '%s\n' "${ALL_TARGETS[@]}" | grep -qx "$target"; then
      unreleased+=("$name")
    fi
  done
  if [[ "${#unmapped[@]}" -gt 0 ]]; then
    echo "Workspace package(s) missing from the npm release inventory: ${unmapped[*]}" >&2
    echo "Add each package to pkg_dir / pkg_target_for_name." >&2
    exit 1
  fi
  if [[ "${#unreleased[@]}" -gt 0 ]]; then
    echo "Workspace package(s) missing from the --package all list: ${unreleased[*]}" >&2
    echo "Add each package to ALL_TARGETS in dependency order." >&2
    exit 1
  fi
}

# The release workflows carry their own NPM_PACKAGES list, used to verify
# versions and to create each package's git tag. A package missing there is
# published untagged and unverified, so hold the two lists to each other.
verify_workflow_inventory() {
  local expected workflow actual
  expected="$(
    for target in "${ALL_TARGETS[@]}"; do
      echo "lingxia-$target"
    done | sort
  )"
  for workflow in "$ROOT_DIR/.github/workflows/npm-release.yml" \
                  "$ROOT_DIR/.github/workflows/create-release-tag.yml"; do
    [[ -f "$workflow" ]] || continue
    actual="$(awk '/^  NPM_PACKAGES:/{f=1; next} f && /^    [a-z]/{print $1; next} f{exit}' "$workflow" | sort)"
    if [[ "$expected" != "$actual" ]]; then
      echo "NPM_PACKAGES in $(basename "$workflow") is out of sync with ALL_TARGETS:" >&2
      diff <(echo "$expected") <(echo "$actual") >&2 || true
      exit 1
    fi
  done
}

pkg_target_for_name() {
  case "$1" in
    @lingxia/bridge) echo "bridge" ;;
    @lingxia/polyfills) echo "polyfills" ;;
    @lingxia/types) echo "types" ;;
    @lingxia/test) echo "test" ;;
    @lingxia/elements) echo "elements" ;;
    @lingxia/page-runtime) echo "page-runtime" ;;
    @lingxia/html) echo "html" ;;
    @lingxia/react) echo "react" ;;
    @lingxia/vue) echo "vue" ;;
    @lingxia/terminal-settings) echo "terminal-settings" ;;
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
  test: "packages/lingxia-test",
  elements: "packages/lingxia-elements",
  "page-runtime": "packages/lingxia-page-runtime",
  html: "packages/lingxia-html",
  react: "packages/lingxia-react",
  vue: "packages/lingxia-vue",
  "terminal-settings": "packages/lingxia-terminal-settings",
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
verify_workflow_inventory

if [[ "$VERIFY_ONLY" -eq 1 ]]; then
  echo "npm release inventory is in sync"
  exit 0
fi
if [[ "$PACKAGE_SET" == "all" ]]; then
  verify_publish_order
fi

# Install the packages/ workspace before building any member. Members carry no
# lockfile of their own: their dev tooling is hoisted to packages/node_modules,
# and the in-repo @lingxia/* links live there too. A member installed on its
# own therefore still cannot build -- lingxia-terminal-settings runs the CLI,
# whose build.rs resolves rolldown from this workspace.
install_packages_workspace() {
  local workspace="$ROOT_DIR/packages"
  [[ -f "$workspace/package.json" ]] || return 0
  echo "==> npm install (packages/ workspace)"
  if [[ -f "$workspace/package-lock.json" ]]; then
    (cd "$workspace" && npm ci)
  else
    (cd "$workspace" && npm install)
  fi
}

install_packages_workspace

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
