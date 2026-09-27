#!/usr/bin/env bash
# The apple SDK's Swift package names lingxia-sdk/apple/Sources/generated/
# {LingXiaRustAPI,LingXiaSwiftAPI} as targets, and SwiftPM checks them when it
# plans, before any build plugin runs. Both are written by the build scripts of
# `lingxia` and `lingxia-platform`, so a fresh checkout has neither. Source this,
# then `ensure_swift_bridge <repo-root>`: it generates them when they are
# missing and leaves them alone otherwise (a later cargo build keeps them
# current).

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

ensure_swift_bridge() {
  local root="$1" cargo="${CARGO_BIN:-cargo}" target
  swift_bridge_missing "$root" || return 0

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
}
