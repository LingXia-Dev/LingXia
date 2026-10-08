#!/usr/bin/env bash
# Cargo succeeds when a stale filter selects no tests. Required CI suites must
# execute at least one test, including when all matches were marked ignored.
set -euo pipefail

result=$(mktemp)
trap 'rm -f "$result"' EXIT
CARGO_TERM_COLOR=never "${CARGO_BIN:-cargo}" test "$@" 2>&1 | tee "$result"
if ! grep -Eq '^test result: ok\. [1-9][0-9]* passed;' "$result"; then
  echo "ERROR: required cargo test invocation passed no tests: $*" >&2
  exit 1
fi
