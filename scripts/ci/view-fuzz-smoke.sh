#!/usr/bin/env bash
# Short libFuzzer runs of every View fuzz target from its committed seeds; any crash,
# timeout or leak fails. Needs nightly and cargo-fuzz.
#   scripts/ci/view-fuzz-smoke.sh [seconds-per-target]
set -euo pipefail
seconds="${1:-30}"
fuzz="$(cd "$(dirname "$0")/../../crates/lingxia-view/fuzz" && pwd)"
cd "$fuzz"
cargo +nightly fuzz build
for target in $(cargo +nightly fuzz list); do
  corpus="$(mktemp -d)"
  if [[ -d "seeds/$target" ]]; then cp "seeds/$target/"* "$corpus/"; fi
  echo "fuzz $target for ${seconds}s"
  cargo +nightly fuzz run "$target" "$corpus" -- -max_total_time="$seconds" -timeout=10 -rss_limit_mb=4096
  rm -rf "$corpus"
done
