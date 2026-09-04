#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

require_path() {
  [[ -e "$ROOT_DIR/$1" ]] || {
    echo "missing control-security gate input: $1" >&2
    exit 1
  }
}

verify_inputs() {
  local paths=(
    crates/lingxia-webview/Cargo.toml
    crates/lingxia-browser/Cargo.toml
    crates/lingxia-lxapp/Cargo.toml
    crates/lingxia-logic/Cargo.toml
    crates/lingxia-app-context/Cargo.toml
    crates/lingxia-settings/Cargo.toml
    crates/lingxia-transfer/Cargo.toml
    crates/lingxia/Cargo.toml
    crates/lingxia/tests/fixtures/authority-escape/Cargo.toml
    crates/lingxia/tests/fixtures/authority-escape/src/main.rs
    packages/lingxia-bridge/package.json
    scripts/ci/authority-escape-gate.sh
    lingxia-sdk/android/gradlew
    lingxia-sdk/android/lingxia/src/test/java/com/lingxia/webview/AndroidDocumentBridgeStateTest.java
    lingxia-sdk/android/lingxia/src/test/java/com/lingxia/webview/DocumentCommitCallbackPolicyTest.java
    lingxia-sdk/android/lingxia/src/test/java/com/lingxia/webview/NativeViewIdBindingTest.java
    lingxia-sdk/android/lingxia/src/test/java/com/lingxia/webview/WebMessageSizePolicyTest.java
    lingxia-sdk/apple/Package.swift
    lingxia-sdk/apple/Tests/LingxiaTests/NativeComponentMessageAdmissionTests.swift
    lingxia-sdk/apple/Tests/LingxiaTests/StaticSettingsSourceTests.swift
    lingxia-sdk/harmony/build.sh
  )
  local path
  for path in "${paths[@]}"; do
    require_path "$path"
  done
}

profile="${1:-}"
case "$profile" in
  verify)
    verify_inputs
    ;;
  portable-rust)
    verify_inputs
    cd "$ROOT_DIR"
    cargo test \
      -p lingxia-webview \
      -p lingxia-browser \
      -p lingxia-lxapp \
      -p lingxia-logic \
      -p lingxia-app-context \
      -p lingxia-settings \
      --lib
    cargo test -p lingxia-transfer --lib download::manager::tests
    cargo test -p lingxia --lib host_addon::tests
    bash scripts/ci/authority-escape-gate.sh run
    ;;
  bridge)
    verify_inputs
    cd "$ROOT_DIR/packages"
    npm test --workspace @lingxia/bridge
    ;;
  android)
    verify_inputs
    cd "$ROOT_DIR"
    ./lingxia-sdk/android/gradlew \
      -p lingxia-sdk/android \
      :lingxia:testDebugUnitTest \
      --tests 'com.lingxia.webview.*' \
      --no-daemon
    ;;
  apple)
    verify_inputs
    require_path lingxia-sdk/apple/Sources/Resources/en.lproj/Localizable.strings
    require_path lingxia-sdk/apple/Sources/Resources/zh-Hans.lproj/Localizable.strings
    cd "$ROOT_DIR"
    host_target="$(rustc -vV | sed -n 's/^host: //p')"
    case "$host_target" in
      aarch64-apple-darwin|x86_64-apple-darwin) ;;
      *) echo "unsupported Apple CI host target: $host_target" >&2; exit 1 ;;
    esac
    cargo build -p lingxia --target "$host_target"
    LINGXIA_BUILD_CONFIG=debug \
      RUNNER_TARGET_TRIPLE="$host_target" \
      swift test --package-path lingxia-sdk/apple
    ;;
  harmony-rust)
    verify_inputs
    cd "$ROOT_DIR"
    cargo test -p lingxia-webview --lib harmony_document::tests
    cargo check -p lingxia-webview --lib --target aarch64-unknown-linux-ohos
    ;;
  harmony-har)
    verify_inputs
    command -v ohpm >/dev/null || { echo "ohpm is required" >&2; exit 1; }
    command -v hvigorw >/dev/null || { echo "hvigorw is required" >&2; exit 1; }
    cd "$ROOT_DIR"
    bash lingxia-sdk/harmony/build.sh --skip-rust
    ;;
  *)
    echo "usage: $0 verify|portable-rust|bridge|android|apple|harmony-rust|harmony-har" >&2
    exit 2
    ;;
esac
