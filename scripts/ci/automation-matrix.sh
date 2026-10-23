#!/usr/bin/env bash

set -euo pipefail

is_true() {
  [[ "${1:-false}" == "true" ]]
}

windows=false
macos=false

if is_true "${FULL:-false}"; then
  windows=true
  macos=true
else
  if is_true "${CROSS_PLATFORM:-false}" \
    || is_true "${WINDOWS:-false}" \
    || is_true "${WINDOWS_ALL:-false}" \
    || is_true "${FRONTEND_SHARED:-false}" \
    || is_true "${REACT:-false}"; then
    windows=true
  fi

  if is_true "${CROSS_PLATFORM:-false}" \
    || is_true "${MACOS:-false}" \
    || is_true "${MACOS_ALL:-false}" \
    || is_true "${FRONTEND_SHARED:-false}" \
    || is_true "${REACT:-false}"; then
    macos=true
  fi
fi

entries=()

append_job() {
  local platform="$1"
  local os="$2"
  local exe="$3"
  local enabled="$4"
  is_true "$enabled" || return 0

  entries+=("{\"platform\":\"$platform\",\"os\":\"$os\",\"exe\":\"$exe\",\"profile\":\"showcase\"}")
}

append_job windows windows-latest .exe "$windows"
append_job macos macos-latest '' "$macos"

if [[ "${#entries[@]}" -gt 0 ]]; then
  echo "automation=true"
  joined=$(IFS=,; echo "${entries[*]}")
  matrix="{\"include\":[$joined]}"
else
  echo "automation=false"
  # GitHub expands the matrix before evaluating the job condition.
  matrix='{"include":[{"platform":"windows","os":"windows-latest","exe":".exe","profile":"skipped"}]}'
fi
echo "automation_matrix=$matrix"
