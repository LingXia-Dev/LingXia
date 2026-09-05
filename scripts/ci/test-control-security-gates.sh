#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
GATE="$ROOT_DIR/scripts/ci/control-security-gates.sh"
AUTHORITY_GATE="$ROOT_DIR/scripts/ci/authority-escape-gate.sh"
WORKFLOW="$ROOT_DIR/.github/workflows/ci.yml"

filter_has_path() {
  local filter="$1"
  local path="$2"
  awk -v header="            ${filter}:" -v entry="              - '${path}'" '
    $0 == header { in_filter = 1; next }
    in_filter && /^            [[:alnum:]_]+:/ { exit(found ? 0 : 1) }
    in_filter && $0 == entry { found = 1 }
    END { exit(found ? 0 : 1) }
  ' "$WORKFLOW"
}

job_has_text() {
  local job="$1"
  local needle="$2"
  awk -v header="  ${job}:" -v needle="$needle" '
    $0 == header { in_job = 1; next }
    in_job && /^  [[:alnum:]_-]+:/ { exit(found ? 0 : 1) }
    in_job && index($0, needle) { found = 1 }
    END { exit(found ? 0 : 1) }
  ' "$WORKFLOW"
}

bash -n "$GATE"
bash -n "$AUTHORITY_GATE"
bash "$GATE" verify
bash "$AUTHORITY_GATE" self-test

grep -Fq 'bash scripts/ci/authority-escape-gate.sh run' "$GATE" || {
  echo "portable Rust security gate does not run the authority escape fixture" >&2
  exit 1
}

grep -Fq 'cargo test -p lingxia-browser --lib policy::tests' "$GATE" || {
  echo "portable Rust security gate does not run the browser navigation policy tests" >&2
  exit 1
}

grep -Fq "cargo rustc -p lingxia --target \"\$host_target\" --lib -- --crate-type=staticlib" "$GATE" || {
  echo "Apple security gate does not build the static library required by SwiftPM" >&2
  exit 1
}

for filter in core control_security; do
  filter_has_path "$filter" 'third_party/**' || {
    echo "CI filter $filter does not cover publishable third_party crates" >&2
    exit 1
  }
done

job_has_text release-tooling 'scripts/release/verify-crate-inventory.test.mjs' || {
  echo "Release tooling CI does not run the crate inventory self-test" >&2
  exit 1
}

grep -Fq 'suite: [portable-rust, bridge]' "$WORKFLOW" || {
  echo "CI workflow does not declare the portable security matrix" >&2
  exit 1
}
grep -Fq "bash scripts/ci/control-security-gates.sh \${{ matrix.suite }}" "$WORKFLOW" || {
  echo "CI workflow does not dispatch the portable security matrix" >&2
  exit 1
}

for profile in android apple harmony-rust harmony-har; do
  grep -Fq "bash scripts/ci/control-security-gates.sh $profile" "$WORKFLOW" || {
    echo "CI workflow does not invoke control-security profile: $profile" >&2
    exit 1
  }
done

for required_job in control-security sdk-apple harmony-native harmony-har; do
  grep -Fq "      - $required_job" "$WORKFLOW" || {
    echo "CI Success does not gate job: $required_job" >&2
    exit 1
  }
done

# An unset Actions variable is the safe default: the job-level condition is
# evaluated before runner allocation. Keeping harmony-har in ci-success.needs
# makes an enabled build failure/cancellation blocking while a default skip is
# accepted by the existing aggregate-result policy.
expected_har_if="    if: \${{ needs.changes.outputs.harmony == 'true' && vars.HARMONY_HAR_CI_ENABLED == 'true' && (github.event_name != 'pull_request' || github.event.pull_request.head.repo.full_name == github.repository) }}"
grep -Fqx "$expected_har_if" "$WORKFLOW" || {
  echo "Harmony HAR must be explicitly enabled before requesting self-hosted" >&2
  exit 1
}
grep -Fq "contains(needs.*.result, 'failure')" "$WORKFLOW" || {
  echo "CI Success does not propagate an enabled Harmony HAR failure" >&2
  exit 1
}
grep -Fq "contains(needs.*.result, 'cancelled')" "$WORKFLOW" || {
  echo "CI Success does not propagate an enabled Harmony HAR cancellation" >&2
  exit 1
}
