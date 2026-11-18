#!/usr/bin/env bash
# The apple SDK's Swift package names lingxia-sdk/apple/Sources/generated/
# {LingXiaRustAPI,LingXiaSwiftAPI} as targets, and SwiftPM checks them when it
# plans, before any build plugin runs. Both are written by the build scripts of
# `lingxia` and `lingxia-platform`, so a fresh checkout has neither. Source this,
# then `ensure_swift_bridge <repo-root>`: it generates them when they are
# missing or older than a bridge source. SwiftPM compiles the SDK against them
# before the plugin's cargo build could rewrite them, so a stale copy fails the
# build on any new bridge function.

swift_bridge_outputs() {
  local generated="$1/lingxia-sdk/apple/Sources/generated"
  printf '%s\n' \
    "$generated/SwiftBridgeCore.h" \
    "$generated/LingXiaRustAPI/module.modulemap" \
    "$generated/LingXiaSwiftAPI/module.modulemap"
}

swift_bridge_missing() {
  local output
  while IFS= read -r output; do
    [[ -f "$output" ]] || return 0
  done < <(swift_bridge_outputs "$1")
  return 1
}

swift_bridge_sources() {
  printf '%s\n' \
    "$1/crates/lingxia/src/ffi/apple.rs" \
    "$1/crates/lingxia/src/ffi/apple/gpu_page.rs" \
    "$1/crates/lingxia-platform/src/apple/ffi.rs"
}

swift_bridge_stale() {
  local root="$1" output source
  while IFS= read -r output; do
    while IFS= read -r source; do
      [[ "$source" -nt "$output" ]] && return 0
    done < <(swift_bridge_sources "$root")
  done < <(swift_bridge_swift "$root")
  return 1
}

swift_bridge_swift() {
  local generated="$1/lingxia-sdk/apple/Sources/generated"
  printf '%s\n' \
    "$generated/LingXiaRustAPI/LingXiaRustAPI/LingXiaRustAPI.swift" \
    "$generated/LingXiaSwiftAPI/LingXiaSwiftAPI/LingXiaSwiftAPI.swift"
}

ensure_swift_bridge() {
  local root="$1" cargo="${CARGO_BIN:-cargo}" target
  swift_bridge_missing "$root" || swift_bridge_stale "$root" || return 0

  case "$(uname -m)" in
    arm64 | aarch64) target=aarch64-apple-darwin ;;
    x86_64) target=x86_64-apple-darwin ;;
    *)
      echo "ERROR: no macOS Rust target for $(uname -m)" >&2
      return 1
      ;;
  esac

  echo "==> Generating the Swift bridge (lingxia-sdk/apple/Sources/generated)" >&2
  # Cargo reruns a build script only when one of its inputs changed, and the
  # generated directory is not one: touch the bridge sources so a cached build
  # writes it again.
  touch "$root/crates/lingxia/src/ffi/apple.rs" "$root/crates/lingxia-platform/src/apple/ffi.rs"
  (cd "$root" && LINGXIA_GENERATE_BRIDGE=1 "$cargo" check -p lingxia -p lingxia-platform --target "$target") || return 1

  if swift_bridge_missing "$root"; then
    echo "ERROR: cargo check did not generate the Swift bridge:" >&2
    swift_bridge_outputs "$root" >&2
    return 1
  fi
  # An unchanged output keeps its old time; mark it current so the next run skips this.
  swift_bridge_swift "$root" | while IFS= read -r output; do touch "$output"; done
}
