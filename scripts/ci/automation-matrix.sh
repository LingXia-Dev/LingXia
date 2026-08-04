#!/usr/bin/env bash

set -euo pipefail

is_true() {
  [[ "${1:-false}" == "true" ]]
}

windows_react=false
windows_vue=false

if is_true "${FULL:-false}"; then
  windows_react=true
  windows_vue=true
else
  if is_true "${CROSS_PLATFORM:-false}" \
    || is_true "${MACOS:-false}" \
    || is_true "${WINDOWS:-false}" \
    || is_true "${REACT:-false}"; then
    windows_react=true
  fi
  if is_true "${MACOS_ALL:-false}" \
    || is_true "${WINDOWS_ALL:-false}" \
    || is_true "${FRONTEND_SHARED:-false}"; then
    windows_react=true
    windows_vue=true
  fi
  if is_true "${VUE:-false}"; then
    windows_vue=true
  fi
fi

entries=()

append_framework() {
  local framework="$1"
  local enabled="$2"
  is_true "$enabled" || return 0

  entries+=("{\"platform\":\"windows\",\"os\":\"windows-latest\",\"exe\":\".exe\",\"framework\":\"$framework\",\"profile\":\"$framework\"}")
}

append_framework react "$windows_react"
append_framework vue "$windows_vue"

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
