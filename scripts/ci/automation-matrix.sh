#!/usr/bin/env bash

set -euo pipefail

is_true() {
  [[ "${1:-false}" == "true" ]]
}

macos_react=false
macos_vue=false
windows_react=false
windows_vue=false

if is_true "${FULL:-false}"; then
  macos_react=true
  macos_vue=true
  windows_react=true
  windows_vue=true
else
  if is_true "${CROSS_PLATFORM:-false}"; then
    macos_react=true
    windows_react=true
  fi
  if is_true "${MACOS:-false}"; then
    macos_react=true
  fi
  if is_true "${MACOS_ALL:-false}"; then
    macos_react=true
    macos_vue=true
  fi
  if is_true "${WINDOWS:-false}"; then
    windows_react=true
  fi
  if is_true "${WINDOWS_ALL:-false}"; then
    windows_react=true
    windows_vue=true
  fi
  if is_true "${FRONTEND_SHARED:-false}"; then
    macos_react=true
    macos_vue=true
  fi
  if is_true "${REACT:-false}"; then
    macos_react=true
  fi
  if is_true "${VUE:-false}"; then
    macos_vue=true
  fi
fi

matrix='{"include":[]}'

append_platform() {
  local platform="$1"
  local os="$2"
  local exe="$3"
  local react_enabled="$4"
  local vue_enabled="$5"
  local frameworks=""

  if is_true "$react_enabled"; then
    frameworks="react"
  fi
  if is_true "$vue_enabled"; then
    frameworks="${frameworks:+$frameworks }vue"
  fi
  [[ -n "$frameworks" ]] || return 0

  matrix=$(jq -c \
    --arg platform "$platform" \
    --arg os "$os" \
    --arg exe "$exe" \
    --arg frameworks "$frameworks" \
    '.include += [{
      platform: $platform,
      os: $os,
      exe: $exe,
      frameworks: $frameworks,
      profile: ($frameworks | gsub(" "; "-"))
    }]' <<<"$matrix")
}

append_platform macos macos-latest "" "$macos_react" "$macos_vue"
append_platform windows windows-latest ".exe" "$windows_react" "$windows_vue"

if [[ "$(jq '.include | length' <<<"$matrix")" -gt 0 ]]; then
  echo "automation=true"
else
  echo "automation=false"
  # GitHub expands a matrix before starting a job. Keep the skipped job's
  # matrix structurally non-empty so a docs-only change cannot fail workflow
  # planning on an empty include list.
  matrix='{"include":[{"platform":"macos","os":"macos-latest","exe":"","frameworks":"react","profile":"skipped"}]}'
fi
echo "automation_matrix=$matrix"
