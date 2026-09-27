#!/usr/bin/env bash
# ensure_swift_bridge (scripts/lib/swift-bridge.sh) against a fake cargo: it
# generates the bridge a fresh checkout lacks, and only then.

set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd "$script_dir/../.." && pwd)
# shellcheck source=../lib/swift-bridge.sh
source "$repo_root/scripts/lib/swift-bridge.sh"

fail() {
  echo "swift bridge case failed: $*" >&2
  exit 1
}

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

root="$work/repo"
mkdir -p "$root/crates/lingxia/src/ffi" "$root/crates/lingxia-platform/src/apple"
touch "$root/crates/lingxia/src/ffi/apple.rs" "$root/crates/lingxia-platform/src/apple/ffi.rs"

# Records how it was called; writes the bridge unless FAKE_CARGO_WRITES=0.
fake_cargo="$work/cargo"
cat >"$fake_cargo" <<'CARGO'
#!/usr/bin/env bash
echo "$PWD|${LINGXIA_GENERATE_BRIDGE:-}|$*" >>"$FAKE_CARGO_LOG"
if [[ "${FAKE_CARGO_WRITES:-1}" == 1 ]]; then
  generated=lingxia-sdk/apple/Sources/generated
  mkdir -p "$generated/LingXiaRustAPI" "$generated/LingXiaSwiftAPI"
  touch "$generated/SwiftBridgeCore.h" "$generated/LingXiaRustAPI/module.modulemap" \
    "$generated/LingXiaSwiftAPI/module.modulemap"
fi
CARGO
chmod +x "$fake_cargo"
export CARGO_BIN="$fake_cargo" FAKE_CARGO_LOG="$work/cargo.log"

# A fresh checkout: generated once, by a check of both crates for this Mac.
ensure_swift_bridge "$root" 2>/dev/null || fail "a fresh checkout must succeed"
[[ $(wc -l <"$FAKE_CARGO_LOG") -eq 1 ]] || fail "cargo must run once"
IFS='|' read -r dir generate args <"$FAKE_CARGO_LOG"
[[ "$dir" == "$root" ]] || fail "cargo must run in the repo root (ran in $dir)"
[[ "$generate" == 1 ]] || fail "LINGXIA_GENERATE_BRIDGE must be set"
[[ "$args" =~ ^check\ -p\ lingxia\ -p\ lingxia-platform\ --target\ (aarch64|x86_64)-apple-darwin$ ]] ||
  fail "unexpected cargo arguments: $args"

# Present: nothing to do.
ensure_swift_bridge "$root" 2>/dev/null || fail "a generated checkout must succeed"
[[ $(wc -l <"$FAKE_CARGO_LOG") -eq 1 ]] || fail "cargo must not run when the bridge exists"

# One output missing is a missing bridge.
rm "$root/lingxia-sdk/apple/Sources/generated/LingXiaSwiftAPI/module.modulemap"
ensure_swift_bridge "$root" 2>/dev/null || fail "a partial bridge must be regenerated"
[[ $(wc -l <"$FAKE_CARGO_LOG") -eq 2 ]] || fail "cargo must run for a partial bridge"

# A build that writes nothing is an error, not a later SwiftPM failure.
rm -rf "$root/lingxia-sdk"
if FAKE_CARGO_WRITES=0 ensure_swift_bridge "$root" 2>"$work/err"; then
  fail "a cargo check that generates nothing must fail"
fi
grep -q "did not generate the Swift bridge" "$work/err" || fail "the failure must say what is missing"

# The Runner install script generates it before its Swift build.
installer="$repo_root/tools/lingxia-runner/macos/install-local-runner.sh"
generate_at=$(grep -n '^ensure_swift_bridge "\$ROOT_DIR"' "$installer" | cut -d: -f1)
build_at=$(grep -n '"\$LINGXIA_BIN" build$' "$installer" | cut -d: -f1)
[[ -n "$generate_at" && -n "$build_at" && "$generate_at" -lt "$build_at" ]] ||
  fail "install-local-runner.sh must generate the Swift bridge before building the Runner"

echo "swift bridge cases passed"
