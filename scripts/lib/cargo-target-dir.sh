#!/usr/bin/env bash
# Resolve Cargo's target directory the same way LingXia's CLI does for a host
# project: explicit CARGO_TARGET_DIR wins, otherwise defer to cargo metadata so
# build.target-dir from Cargo config is honored.

resolve_cargo_target_dir() {
  local project_root="$1" manifest_path="${2:-$1/Cargo.toml}"

  if [[ -n "${CARGO_TARGET_DIR:-}" ]]; then
    case "$CARGO_TARGET_DIR" in
      /*) printf '%s\n' "$CARGO_TARGET_DIR" ;;
      *) printf '%s\n' "$project_root/$CARGO_TARGET_DIR" ;;
    esac
    return 0
  fi

  local cargo_bin="${CARGO_BIN:-cargo}" metadata target_dir
  metadata="$("$cargo_bin" metadata --no-deps --format-version 1 --manifest-path "$manifest_path")"
  target_dir="$(
    printf '%s\n' "$metadata" |
      sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p' |
      head -n 1
  )"

  if [[ -n "$target_dir" ]]; then
    printf '%s\n' "$target_dir"
  else
    printf '%s\n' "$project_root/target"
  fi
}
