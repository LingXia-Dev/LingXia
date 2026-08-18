#!/usr/bin/env bash
# Put `lingxia` and `lxdev` in --dest using, in order:
#   1. dest already matching this tree's CLI fingerprint (GHA cache hit)
#   2. GitHub Release lingxia-cli-v* whose tree fingerprint matches
#   3. cargo build from this checkout
#
# Linux has no published CLI assets; step 2 is skipped there.

set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd "$script_dir/../.." && pwd)

dest=""
profile=debug

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dest) dest=${2:-}; shift ;;
    --profile) profile=${2:-}; shift ;;
    -h|--help)
      echo "usage: resolve-cli.sh [--dest DIR] [--profile debug|release]"
      exit 0
      ;;
    *)
      echo "unknown option: $1" >&2
      exit 2
      ;;
  esac
  shift
done

if [[ "$profile" != "debug" && "$profile" != "release" ]]; then
  echo "profile must be debug or release" >&2
  exit 2
fi

if [[ -z "$dest" ]]; then
  dest="$repo_root/target/$profile"
fi

exe=""
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*|Windows_NT) exe=".exe" ;;
esac

lingxia="$dest/lingxia$exe"
lxdev="$dest/lxdev$exe"
stamp="$dest/.lingxia-cli-fingerprint"

fingerprint_now=$(bash "$script_dir/cli-fingerprint.sh" HEAD)

have_bins() {
  [[ -x "$lingxia" && -x "$lxdev" ]] || [[ -f "$lingxia" && -f "$lxdev" ]]
}

stamp_matches() {
  [[ -f "$stamp" && "$(cat "$stamp")" == "$fingerprint_now" ]]
}

write_stamp() {
  mkdir -p "$dest"
  printf '%s\n' "$fingerprint_now" > "$stamp"
}

report() {
  echo "cli_source=$1"
  echo "cli_dest=$dest"
  echo "cli_fingerprint=$fingerprint_now"
  if [[ -n "${2:-}" ]]; then
    echo "cli_tag=$2"
  fi
}

if have_bins && stamp_matches; then
  report cache
  exit 0
fi

latest_cli_tag() {
  git tag -l 'lingxia-cli-v*' --sort=-v:refname | head -n 1
}

ensure_cli_tags() {
  if [[ -n "$(latest_cli_tag)" ]]; then
    return 0
  fi
  git fetch --depth=1 --force origin 'refs/tags/lingxia-cli-v*:refs/tags/lingxia-cli-v*' >/dev/null 2>&1 || true
}

try_release() {
  case "$(uname -s)" in
    Linux) return 1 ;;
  esac
  ensure_cli_tags
  local tag
  tag=$(latest_cli_tag)
  [[ -n "$tag" ]] || return 1
  if ! git cat-file -e "$tag^{commit}" 2>/dev/null; then
    git fetch --depth=1 --force origin "refs/tags/$tag:refs/tags/$tag" >/dev/null 2>&1 || return 1
  fi
  local tag_fp
  tag_fp=$(bash "$script_dir/cli-fingerprint.sh" "$tag")
  [[ "$tag_fp" == "$fingerprint_now" ]] || return 1

  local version=${tag#lingxia-cli-v}
  echo "CLI fingerprint matches $tag; installing published binaries..." >&2
  mkdir -p "$dest"
  if ! (
    cd "$repo_root"
    LINGXIA_VERSION="$version" LINGXIA_INSTALL_DIR="$dest" bash install.sh
  ); then
    echo "published CLI install failed; falling back to cargo build" >&2
    return 1
  fi
  write_stamp
  report release "$tag"
}

if try_release; then
  exit 0
fi

echo "Building lingxia + lxdev from this checkout ($profile)..." >&2
(
  cd "$repo_root"
  if [[ "$profile" == "release" ]]; then
    cargo build -p lingxia-cli -p lingxia-devtools-cli --release
  else
    cargo build -p lingxia-cli -p lingxia-devtools-cli
  fi
)
mkdir -p "$dest"
built_dir="$repo_root/target/$profile"
if [[ "$(cd "$dest" && pwd)" != "$(cd "$built_dir" && pwd)" ]]; then
  cp "$built_dir/lingxia$exe" "$lingxia"
  cp "$built_dir/lxdev$exe" "$lxdev"
fi
write_stamp
report build
