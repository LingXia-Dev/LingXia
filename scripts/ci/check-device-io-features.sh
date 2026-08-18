#!/usr/bin/env bash
set -euo pipefail

scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT

assert_has() {
  local file="$1"
  local needle="$2"
  local description="$3"
  if ! grep -Fq "$needle" "$file"; then
    echo "missing feature boundary: $description" >&2
    exit 1
  fi
}

assert_lacks() {
  local file="$1"
  local needle="$2"
  local description="$3"
  if grep -Fq "$needle" "$file"; then
    echo "violated feature boundary: $description" >&2
    exit 1
  fi
}

cargo tree -e features -p lingxia --no-default-features --features automation \
  > "$scratch/automation"
assert_lacks "$scratch/automation" "lingxia-device-io" \
  "base lingxia/automation must not enable desktop device I/O"

cargo tree -e features -p lingxia-device-io --no-default-features \
  --features diagnostics > "$scratch/diagnostics"
cargo tree -e features -p lingxia-device-io --no-default-features \
  --features diagnostics -i lingxia-device-io > "$scratch/diagnostics-features"
assert_lacks "$scratch/diagnostics-features" 'lingxia-device-io feature "snapshot"' \
  "device diagnostics must not enable snapshot"
assert_lacks "$scratch/diagnostics" "image v" \
  "device diagnostics must not include snapshot image encoding"

cargo tree -e features -p lingxia-device-io --no-default-features \
  --features process > "$scratch/process"
cargo tree -e features -p lingxia-device-io --no-default-features \
  --features process -i lingxia-device-io > "$scratch/process-features"
assert_lacks "$scratch/process-features" 'lingxia-device-io feature "window"' \
  "process inspection must not enable window automation"
assert_lacks "$scratch/process" "image v" \
  "process inspection must not include snapshot image encoding"

cargo tree -e features -p lingxia-device-io --no-default-features \
  --features input -i lingxia-device-io > "$scratch/input-features"
assert_lacks "$scratch/input-features" 'lingxia-device-io feature "window"' \
  "synthetic input must not expose window enumeration or management"

cargo tree -e features -p lingxia-device-io --no-default-features \
  --features clipboard -i lingxia-device-io > "$scratch/clipboard-features"
assert_has "$scratch/clipboard-features" 'lingxia-device-io feature "input"' \
  "clipboard paste must include synthetic keyboard input"
assert_lacks "$scratch/clipboard-features" 'lingxia-device-io feature "window"' \
  "clipboard access must not expose window enumeration or management"

cargo tree -e features -p lingxia --no-default-features --features desktop-automation \
  > "$scratch/desktop-automation"
cargo tree -e features -p lingxia --no-default-features --features desktop-automation \
  -i lingxia-device-io > "$scratch/desktop-automation-features"
assert_has "$scratch/desktop-automation-features" 'lingxia-device-io feature "snapshot"' \
  "lingxia/desktop-automation must include snapshot"
assert_lacks "$scratch/desktop-automation-features" 'lingxia-device-io feature "supervision"' \
  "lingxia/desktop-automation must not include host supervision"
assert_lacks "$scratch/desktop-automation-features" 'lingxia-device-io feature "wire"' \
  "in-process desktop automation must not include transport DTOs"

cargo tree -e features -p lingxia-control-commands --no-default-features \
  > "$scratch/control-commands"
assert_lacks "$scratch/control-commands" "lingxia-device-io" \
  "base control commands must not depend on device I/O"

# The command table forwards to the host; only `lxdev` runs the verbs itself.
# A product that takes the client must not link a backend it cannot reach.
cargo tree -e features -p lingxia-control-commands --no-default-features \
  --features desktop > "$scratch/desktop-client"
cargo tree -e features -p lingxia-control-commands --no-default-features \
  --features desktop -i lingxia-device-io > "$scratch/desktop-client-features"
assert_has "$scratch/desktop-client-features" 'lingxia-device-io feature "wire"' \
  "the desktop command client must include transport DTOs"
for capability in app ax clipboard diagnostics input process snapshot window; do
  assert_lacks "$scratch/desktop-client-features" \
    "lingxia-device-io feature \"$capability\"" \
    "the desktop command client must not include the $capability implementation"
done
assert_lacks "$scratch/desktop-client" "objc2 v" \
  "the desktop command client must not link macOS device bindings"

cargo tree -e features -p lingxia-control-commands --no-default-features \
  --features desktop-local -i lingxia-device-io > "$scratch/desktop-local-features"
assert_has "$scratch/desktop-local-features" 'lingxia-device-io feature "input"' \
  "desktop-local must include the synthetic input implementation"
assert_lacks "$scratch/desktop-local-features" 'lingxia-device-io feature "supervision"' \
  "desktop-local is a development mount and must not take host supervision"

# `computerUse` is one declared capability, so one SDK feature has to carry
# both halves — the product command and the in-process automation driver.
# Splitting them across two features people enable by hand is how a host ends
# up answering a capability it only half has.
cargo tree -e features -p lingxia --no-default-features --features computer-use \
  -i lingxia-device-io > "$scratch/product-computer-use"
assert_has "$scratch/product-computer-use" 'lingxia-device-io feature "input"' \
  "lingxia/computer-use must bring the in-process desktop automation driver"

cargo tree -e features -p lingxia-devtools-cli > "$scratch/lxdev"
cargo tree -e features -p lingxia-devtools-cli -i lingxia-device-io \
  > "$scratch/lxdev-features"
assert_has "$scratch/lxdev-features" 'lingxia-device-io feature "snapshot"' \
  "lxdev desktop must include snapshot"
assert_has "$scratch/lxdev-features" 'lingxia-device-io feature "wire"' \
  "lxdev desktop must include command DTOs"
assert_lacks "$scratch/lxdev-features" 'lingxia-device-io feature "supervision"' \
  "standalone lxdev must not include host supervision"
assert_lacks "$scratch/lxdev" "lingxia-platform" \
  "standalone lxdev must not include the platform host runtime"

cargo tree -e features -p lingxia-control-runtime --no-default-features \
  --features test-runtime > "$scratch/test-runtime"
assert_lacks "$scratch/test-runtime" "lingxia-device-io" \
  "test runtime without computer-use must not enable desktop device I/O"

for runner in lingxia-runner-lib lingxia-runner-windows; do
  cargo tree -e features -p "$runner" -i lingxia-device-io \
    > "$scratch/$runner-features"
  assert_has "$scratch/$runner-features" 'lingxia-device-io feature "snapshot"' \
    "$runner must preserve desktop automation for its trusted test runtime"
  assert_lacks "$scratch/$runner-features" 'lingxia-device-io feature "supervision"' \
    "$runner desktop automation must not include product-host supervision"
done

cargo tree -e features -p lingxia-control-runtime --no-default-features \
  --features computer-use > "$scratch/control-runtime"
cargo tree -e features -p lingxia-control-runtime --no-default-features \
  --features computer-use -i lingxia-device-io > "$scratch/control-runtime-features"
assert_has "$scratch/control-runtime-features" 'lingxia-device-io feature "snapshot"' \
  "host computer-use must include snapshot"
assert_has "$scratch/control-runtime-features" 'lingxia-device-io feature "supervision"' \
  "host computer-use must include supervision"
assert_has "$scratch/control-runtime-features" 'lingxia-device-io feature "wire"' \
  "host computer-use must include transport DTOs"

cargo tree -e features -p lingxia-media --no-default-features > "$scratch/media-core"
assert_lacks "$scratch/media-core" "lingxia-platform" \
  "lingxia-media without playback must not include platform playback contracts"
assert_lacks "$scratch/media-core" "lingxia-device-io" \
  "lingxia-media must not depend on the desktop device-I/O implementation"

cargo tree -e features -p lingxia-media --features playback > "$scratch/media-playback"
assert_has "$scratch/media-playback" "lingxia-platform" \
  "lingxia-media/playback must include the existing platform contracts"
assert_lacks "$scratch/media-playback" "lingxia-device-io" \
  "lingxia-media/playback must remain independent of desktop device I/O"

cargo tree -e features -p lingxia-media --no-default-features --features capture \
  > "$scratch/media-capture"
cargo tree -e features -p lingxia-media --no-default-features --features capture \
  -i lingxia-platform > "$scratch/media-capture-platform"
assert_has "$scratch/media-capture-platform" 'lingxia-platform feature "capture-contract"' \
  "lingxia-media/capture must take the platform capture contract"
assert_lacks "$scratch/media-capture" "lingxia-device-io" \
  "lingxia-media/capture must not depend on desktop device I/O"

cargo tree -e features -p lingxia-device-io --no-default-features \
  --features snapshot > "$scratch/snapshot"
cargo tree -e features -p lingxia-device-io --no-default-features \
  --features snapshot -i lingxia-device-io > "$scratch/snapshot-features"
assert_has "$scratch/snapshot-features" 'lingxia-device-io feature "desktop-capture-engine"' \
  "snapshot must reuse the shared desktop visual engine"
assert_lacks "$scratch/snapshot-features" 'lingxia-device-io feature "realtime-capture-provider"' \
  "snapshot must not enable the realtime capture provider"
assert_lacks "$scratch/snapshot" "lingxia-platform" \
  "snapshot must not enable the platform capture contract"

cargo tree -e features -p lingxia-device-io --no-default-features \
  --features realtime-capture-provider > "$scratch/realtime-provider"
cargo tree -e features -p lingxia-device-io --no-default-features \
  --features realtime-capture-provider -i lingxia-platform \
  > "$scratch/realtime-provider-platform"
assert_has "$scratch/realtime-provider-platform" 'lingxia-platform feature "capture-contract"' \
  "the desktop realtime adapter must take the capture contract"
