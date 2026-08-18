#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

CRATES=(
  # Foundational crates.
  "lingxia-app-context"
  "lingxia-device-io"
  "lingxia-provider"
  "lingxia-log"
  "lingxia-update"
  "lingxia-messaging"
  "lingxia-webview"
  "lingxia-settings"
  "lingxia-transfer"
  "lingxia-windows-contract"
  "lingxia-surface"
  "lingxia-shell"
  "lingxia-platform"
  "lingxia-media"
  "lingxia-service"

  # Terminal backend has no workspace deps; config sits on app-context +
  # terminal. Both must precede lingxia-logic / lingxia, which take them as
  # optional `terminal` / `terminal-runtime` features.
  "lingxia-terminal"
  "lingxia-terminal-config"

  # Core runtime crates.
  "lingxia-lxapp"
  "lingxia-logic"
  "lingxia-proxy"

  # Facade and host support crates.
  "lingxia-native-macros"
  "lingxia-native-codegen"
  "lingxia-browser"
  "lingxia-automation"
  "lingxia-browser-shell"

  # Control protocol is consumed by SDK/tools and by lingxia-control-runtime.
  "lingxia-control-protocol"

  # Shared command library depends on the protocol and is consumed by lingxia.
  "lingxia-control-commands"

  # Public facade.
  "lingxia"

  # The shared control runtime depends on the public facade.
  "lingxia-control-runtime"

  # Windows build helper consumed by the windows app template.
  # lingxia-windows-sdk is NOT published: it depends on a git-pinned windows-rs
  # (proc-macros), and crates.io forbids git deps. It is consumed via a git ref
  # in the windows app template instead.
  "lingxia-windows-build"
)

# Workspace crates that cannot be published to crates.io. Keep this explicit so
# adding a new crate under crates/ fails the inventory check until the release
# policy is decided.
UNPUBLISHED_CRATES=(
  "lingxia-windows-sdk"
)

array_contains() {
  local needle="$1"
  shift
  local item
  for item in "$@"; do
    [[ "$item" == "$needle" ]] && return 0
  done
  return 1
}

verify_crate_inventory() {
  local manifest crate
  local missing=()

  for manifest in "$ROOT_DIR"/crates/*/Cargo.toml; do
    crate="$(awk '
      /^\[package\]/ {in_package=1; next}
      /^\[/ {in_package=0}
      in_package && $1 == "name" {
        gsub(/"/, "", $3)
        print $3
        exit
      }' "$manifest")"

    if [[ -z "$crate" ]]; then
      echo "Failed to read package name from $manifest" >&2
      exit 1
    fi

    if ! array_contains "$crate" "${CRATES[@]}" &&
       ! array_contains "$crate" "${UNPUBLISHED_CRATES[@]}"; then
      missing+=("$crate")
    fi
  done

  if [[ "${#missing[@]}" -gt 0 ]]; then
    echo "Workspace crate(s) missing from the release inventory: ${missing[*]}" >&2
    echo "Add each crate to CRATES or UNPUBLISHED_CRATES with a reason." >&2
    exit 1
  fi
}

usage() {
  cat <<'EOF'
Release LingXia crates.io packages.

Usage:
  scripts/release/crates.sh [--publish] [--dry-run] [--allow-dirty] [--no-verify] [--from <crate>] [--only <crates>]

Options:
  --publish        Publish crates to crates.io in dependency order.
  --dry-run        Run cargo package checks only.
  --allow-dirty    Pass --allow-dirty to cargo publish.
  --no-verify      Pass --no-verify to cargo publish.
  --from <crate>   Start from the given crate in the publish order.
  --only <crates>  Publish only the listed crates (comma-separated, or repeat the flag).
                   The dependency order from this script is preserved regardless of input order.
                   Mutually exclusive with --from.
  -h, --help       Show help.
EOF
}

PUBLISH=0
DRY_RUN=0
ALLOW_DIRTY=0
NO_VERIFY=0
FROM_CRATE=""
ONLY_CRATES=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --publish) PUBLISH=1 ;;
    --dry-run) DRY_RUN=1 ;;
    --allow-dirty) ALLOW_DIRTY=1 ;;
    --no-verify) NO_VERIFY=1 ;;
    --from)
      shift
      if [[ $# -eq 0 ]]; then
        echo "--from requires a crate name" >&2
        usage
        exit 2
      fi
      FROM_CRATE="$1"
      ;;
    --only)
      shift
      if [[ $# -eq 0 ]]; then
        echo "--only requires one or more crate names (comma-separated)" >&2
        usage
        exit 2
      fi
      IFS=',' read -r -a _only_chunk <<< "$1"
      for _c in "${_only_chunk[@]}"; do
        _c="${_c// /}"
        [[ -n "$_c" ]] && ONLY_CRATES+=("$_c")
      done
      ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown option: $1" >&2; usage; exit 2 ;;
  esac
  shift
done

if [[ -n "$FROM_CRATE" && "${#ONLY_CRATES[@]}" -gt 0 ]]; then
  echo "--from and --only are mutually exclusive" >&2
  exit 2
fi

if [[ "$PUBLISH" -eq 0 && "$DRY_RUN" -eq 0 ]]; then
  DRY_RUN=1
fi

verify_crate_inventory

if [[ "$ALLOW_DIRTY" -eq 0 ]] && ! git -C "$ROOT_DIR" diff --quiet; then
  echo "Working directory has uncommitted changes." >&2
  echo "Commit or stash changes before release, or pass --allow-dirty for local verification." >&2
  exit 1
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

SELECTED_CRATES=("${CRATES[@]}")
if [[ -n "$FROM_CRATE" ]]; then
  start_index=-1
  for i in "${!CRATES[@]}"; do
    if [[ "${CRATES[$i]}" == "$FROM_CRATE" ]]; then
      start_index=$i
      break
    fi
  done

  if [[ "$start_index" -lt 0 ]]; then
    echo "Unknown crate for --from: $FROM_CRATE" >&2
    echo "Known crates: ${CRATES[*]}" >&2
    exit 2
  fi

  SELECTED_CRATES=("${CRATES[@]:$start_index}")
fi

if [[ "${#ONLY_CRATES[@]}" -gt 0 ]]; then
  unknown=()
  for requested in "${ONLY_CRATES[@]}"; do
    found=0
    for known in "${CRATES[@]}"; do
      if [[ "$known" == "$requested" ]]; then
        found=1
        break
      fi
    done
    [[ "$found" -eq 0 ]] && unknown+=("$requested")
  done

  if [[ "${#unknown[@]}" -gt 0 ]]; then
    echo "Unknown crate(s) for --only: ${unknown[*]}" >&2
    echo "Known crates: ${CRATES[*]}" >&2
    exit 2
  fi

  filtered=()
  for known in "${CRATES[@]}"; do
    for requested in "${ONLY_CRATES[@]}"; do
      if [[ "$known" == "$requested" ]]; then
        filtered+=("$known")
        break
      fi
    done
  done
  SELECTED_CRATES=("${filtered[@]}")
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
  if [[ -n "$FROM_CRATE" ]]; then
    echo "Starting from: $FROM_CRATE"
  fi
  if [[ "${#ONLY_CRATES[@]}" -gt 0 ]]; then
    echo "Only: ${SELECTED_CRATES[*]}"
  fi
  for crate in "${SELECTED_CRATES[@]}"; do
    echo "==> cargo package -p $crate --no-verify"
    if [[ "$ALLOW_DIRTY" -eq 1 ]]; then
      cargo package -p "$crate" --no-verify --allow-dirty >/dev/null
    else
      cargo package -p "$crate" --no-verify >/dev/null
    fi
    echo "✓ $crate package check passed"
  done
fi

if [[ "$PUBLISH" -eq 0 ]]; then
  exit 0
fi

if [[ -n "$FROM_CRATE" ]]; then
  echo "Resuming publish order from: $FROM_CRATE"
fi
if [[ "${#ONLY_CRATES[@]}" -gt 0 ]]; then
  echo "Publishing only: ${SELECTED_CRATES[*]}"
fi

for crate in "${SELECTED_CRATES[@]}"; do
  echo ""
  echo "=========================================="
  echo "Publishing $crate@$workspace_version"
  echo "=========================================="

  publish_args=(-p "$crate")
  [[ "$ALLOW_DIRTY" -eq 1 ]] && publish_args+=(--allow-dirty)
  [[ "$NO_VERIFY" -eq 1 ]] && publish_args+=(--no-verify)

  # Retry on crates.io rate limits (HTTP 429). Publishing a brand-new crate
  # name (e.g. lingxia-surface) can trip the per-account "new crate" limit, and
  # the index/CDN occasionally 429s under load. `already uploaded|already
  # exists` stays a clean skip so reruns are idempotent.
  publish_attempts=5
  publish_backoff=30
  status=0
  for attempt in $(seq 1 "$publish_attempts"); do
    set +e
    output="$(cargo publish "${publish_args[@]}" 2>&1)"
    status=$?
    set -e
    echo "$output"

    [[ $status -eq 0 ]] && break

    if echo "$output" | grep -Eq "already uploaded|already exists"; then
      echo "✓ $crate already published, skipping"
      status=0
      break
    fi

    if echo "$output" | grep -Eiq "429|too many requests|rate limit|published too many"; then
      if [[ "$attempt" -lt "$publish_attempts" ]]; then
        echo "Rate limited publishing $crate (attempt ${attempt}/${publish_attempts}); retrying in ${publish_backoff}s..." >&2
        sleep "$publish_backoff"
        publish_backoff=$((publish_backoff * 2))
        continue
      fi
    fi

    echo "✗ Failed to publish $crate" >&2
    exit 1
  done

  wait_for_index "$crate" "$workspace_version"
done

echo ""
echo "✅ All crates processed."
