#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

usage() {
  cat <<'EOF'
Release LingXia npm packages.

Usage:
  scripts/release/npm.sh [--package core|types|components|all] [--publish] [--dry-run]

Options:
  --package <name>  Package set to process (default: all)
  --publish         Publish to npm registry.
  --dry-run         Build + npm pack --dry-run.
  -h, --help        Show help.
EOF
}

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
  core) targets=("core") ;;
  types) targets=("types") ;;
  components) targets=("components") ;;
  all) targets=("core" "types" "components") ;;
  *) echo "Unknown package set: $PACKAGE_SET" >&2; exit 2 ;;
esac

pkg_dir() {
  case "$1" in
    core) echo "$ROOT_DIR/packages/lingxia-core" ;;
    types) echo "$ROOT_DIR/packages/lingxia-types" ;;
    components) echo "$ROOT_DIR/packages/lingxia-components" ;;
    *) return 1 ;;
  esac
}

for target in "${targets[@]}"; do
  dir="$(pkg_dir "$target")"
  name="$(node -p "require('$dir/package.json').name")"
  version="$(node -p "require('$dir/package.json').version")"

  echo ""
  echo "=========================================="
  echo "Processing $name@$version ($target)"
  echo "=========================================="

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

  if npm view "$name@$version" version >/dev/null 2>&1; then
    echo "✓ $name@$version already published, skipping"
    continue
  fi

  if [[ -n "${GITHUB_ACTIONS:-}" ]]; then
    (cd "$dir" && npm publish --access public --provenance)
  else
    (cd "$dir" && npm publish --access public)
  fi
  echo "✓ Published $name@$version"
done

echo ""
echo "✅ npm release script completed."
