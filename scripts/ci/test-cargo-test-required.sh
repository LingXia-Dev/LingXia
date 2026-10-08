#!/usr/bin/env bash
set -euo pipefail
script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cat > "$work/cargo" <<'CARGO'
#!/usr/bin/env bash
[[ "$*" == 'test -p fixture --lib selected' ]] || exit 99
printf '%s\n' "$FAKE_RESULT"
exit "${FAKE_STATUS:-0}"
CARGO
chmod +x "$work/cargo"
export CARGO_BIN="$work/cargo"
run() { bash "$script_dir/cargo-test-required.sh" -p fixture --lib selected > "$work/output" 2>&1; }

export FAKE_RESULT='test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;'
run
export FAKE_STATUS=101
if run; then echo 'Cargo failures must propagate' >&2; exit 1; fi
unset FAKE_STATUS
for FAKE_RESULT in \
  'test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 140 filtered out;' \
  'test result: ok. 0 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out;' \
  'Finished compilation without executing tests'; do
  export FAKE_RESULT
  if run; then echo "Empty suite passed: $FAKE_RESULT" >&2; exit 1; fi
done
echo 'required cargo test cases passed'
