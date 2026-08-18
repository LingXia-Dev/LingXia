#!/usr/bin/env bash
# Print `true` when the CLI fingerprint differs between two refs.
# Usage: cli-inputs-changed.sh <base-ref> [head-ref]

set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)

if [[ $# -lt 1 ]]; then
  echo "usage: cli-inputs-changed.sh <base-ref> [head-ref]" >&2
  exit 2
fi

base=$1
head=${2:-HEAD}

fp() {
  bash "$script_dir/cli-fingerprint.sh" "$1"
}

if [[ "$(fp "$base")" == "$(fp "$head")" ]]; then
  echo false
else
  echo true
fi
