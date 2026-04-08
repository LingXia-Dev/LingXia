#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

CRATES=(
  # Foundational crates.
  "lingxia-app-context"
  "lingxia-provider"
  "lingxia-observability"
  "lingxia-update"
  "lingxia-messaging"
  "lingxia-webview"
  "lingxia-settings"
  "lingxia-platform"
  "lingxia-media"

  # Core runtime crates.
  "lingxia-lxapp"
  "lingxia-transfer"
  "lingxia-logic"

  # Facade support crate required by lingxia.
  "lingxia-macro"

  # Public facade.
  "lingxia"
)

usage() {
  cat <<'EOF'
Release LingXia crates.io packages.

Usage:
  scripts/release/crates.sh [--publish] [--dry-run] [--allow-dirty]

Options:
  --publish       Publish crates to crates.io in dependency order.
  --dry-run       Run cargo package checks only.
  --allow-dirty   Pass --allow-dirty to cargo publish.
  -h, --help      Show help.
EOF
}

PUBLISH=0
DRY_RUN=0
ALLOW_DIRTY=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --publish) PUBLISH=1 ;;
    --dry-run) DRY_RUN=1 ;;
    --allow-dirty) ALLOW_DIRTY=1 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown option: $1" >&2; usage; exit 2 ;;
  esac
  shift
done

if [[ "$PUBLISH" -eq 0 && "$DRY_RUN" -eq 0 ]]; then
  DRY_RUN=1
fi

workspace_version="$(awk '
  /^\[workspace\.package\]/ {in_section=1; next}
  /^\[/ {in_section=0}
  in_section && $1 == "version" {
    gsub(/"/, "", $3);
    print $3;
    exit
  }' "$ROOT_DIR/Cargo.toml")"

if [[ -z "$workspace_version" ]]; then
  echo "Failed to read workspace version from Cargo.toml" >&2
  exit 1
fi

wait_for_index() {
  local crate="$1"
  local version="$2"
  local attempts="${3:-20}"
  local delay_secs="${4:-15}"

  for i in $(seq 1 "$attempts"); do
    if python3 - "$crate" "$version" <<'PY'
import json, sys, urllib.request
crate = sys.argv[1]
version = sys.argv[2]
try:
    with urllib.request.urlopen(f"https://crates.io/api/v1/crates/{crate}") as resp:
        data = json.load(resp)
    versions = [v.get("num") for v in data.get("versions", [])]
    sys.exit(0 if version in versions else 1)
except Exception:
    sys.exit(1)
PY
    then
      echo "✓ ${crate}@${version} indexed"
      return 0
    fi
    echo "Waiting for crates.io index: ${crate}@${version} (${i}/${attempts})..."
    sleep "$delay_secs"
  done
  echo "✗ ${crate}@${version} was not indexed in time" >&2
  return 1
}

cd "$ROOT_DIR"

if [[ "$DRY_RUN" -eq 1 ]]; then
  echo "Dry run: cargo package checks"
  for crate in "${CRATES[@]}"; do
    echo "==> cargo package -p $crate --list"
    if [[ "$ALLOW_DIRTY" -eq 1 ]]; then
      cargo package -p "$crate" --list --allow-dirty >/dev/null
    else
      cargo package -p "$crate" --list >/dev/null
    fi
    echo "✓ $crate package check passed"
  done
fi

if [[ "$PUBLISH" -eq 0 ]]; then
  exit 0
fi

for crate in "${CRATES[@]}"; do
  echo ""
  echo "=========================================="
  echo "Publishing $crate@$workspace_version"
  echo "=========================================="

  set +e
  if [[ "$ALLOW_DIRTY" -eq 1 ]]; then
    output="$(cargo publish -p "$crate" --locked --allow-dirty 2>&1)"
  else
    output="$(cargo publish -p "$crate" --locked 2>&1)"
  fi
  status=$?
  set -e

  echo "$output"

  if [[ $status -ne 0 ]]; then
    if echo "$output" | grep -Eq "already uploaded|already exists"; then
      echo "✓ $crate already published, skipping"
    else
      echo "✗ Failed to publish $crate" >&2
      exit 1
    fi
  fi

  wait_for_index "$crate" "$workspace_version"
done

echo ""
echo "✅ All crates processed."
