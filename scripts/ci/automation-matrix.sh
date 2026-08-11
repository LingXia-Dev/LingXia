#!/usr/bin/env bash

set -euo pipefail

is_true() {
  [[ "${1:-false}" == "true" ]]
}

windows_react=false
windows_vue=false
macos_react=false

if is_true "${FULL:-false}"; then
  windows_react=true
  windows_vue=true
  macos_react=true
else
  if is_true "${CROSS_PLATFORM:-false}" \
    || is_true "${WINDOWS:-false}" \
    || is_true "${REACT:-false}"; then
    windows_react=true
  fi
  if is_true "${WINDOWS_ALL:-false}" \
    || is_true "${FRONTEND_SHARED:-false}"; then
    windows_react=true
    windows_vue=true
  fi
  if is_true "${VUE:-false}"; then
    windows_vue=true
  fi

  if is_true "${CROSS_PLATFORM:-false}" \
    || is_true "${MACOS:-false}" \
    || is_true "${MACOS_ALL:-false}" \
    || is_true "${FRONTEND_SHARED:-false}" \
    || is_true "${REACT:-false}"; then
    macos_react=true
  fi
fi

entries=()

append_framework() {
  local platform="$1"
  local os="$2"
  local exe="$3"
  local framework="$4"
  local enabled="$5"
  is_true "$enabled" || return 0

  entries+=("{\"platform\":\"$platform\",\"os\":\"$os\",\"exe\":\"$exe\",\"framework\":\"$framework\",\"profile\":\"$framework\"}")
}

if is_true "$windows_react" && is_true "$windows_vue"; then
  append_framework windows windows-latest .exe all true
else
  append_framework windows windows-latest .exe react "$windows_react"
  append_framework windows windows-latest .exe vue "$windows_vue"
fi

append_framework macos macos-latest '' react "$macos_react"

if [[ "${#entries[@]}" -gt 0 ]]; then
  echo "automation=true"
  joined=$(IFS=,; echo "${entries[*]}")
  matrix="{\"include\":[$joined]}"
else
  echo "automation=false"
  # GitHub expands the matrix before evaluating the job condition.
  matrix='{"include":[{"platform":"windows","os":"windows-latest","exe":".exe","framework":"react","profile":"skipped"}]}'
fi
echo "automation_matrix=$matrix"
