#!/usr/bin/env bash
# Run both showcases' full suites on one attached device from a clean export
# of a commit, and keep every result under an evidence directory, so a
# readiness round always carries the whole suite rather than chosen rows.
#
#   scripts/automation/device-readiness.sh --platform android --device SERIAL
#   scripts/automation/device-readiness.sh --platform macos [--rev REV] [--keep-export]
#     [--app lingxia-showcase-gpu]   (one showcase only)
#
# Writes <evidence>/<rev>/<app>-<platform>/ (report.json, junit.xml, logs) and
# <evidence>/<rev>/summary.json. Exits non-zero when a suite fails or its
# feature-coverage check does.
set -euo pipefail

platform="" device="" rev="HEAD" keep=0 apps="lingxia-showcase lingxia-showcase-gpu"
evidence="${LINGXIA_EVIDENCE_DIR:-$HOME/.cache/lingxia-gpu-evidence/quality/readiness}"
while (( $# )); do
  case "$1" in
    --platform) platform=$2; shift 2 ;;
    --device) device=$2; shift 2 ;;
    --rev) rev=$2; shift 2 ;;
    --evidence) evidence=$2; shift 2 ;;
    --keep-export) keep=1; shift ;;
    --app) apps=$2; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done
case "$platform" in
  android|macos) ;;
  *) echo "--platform android|macos is required" >&2; exit 2 ;;
esac
[[ "$platform" == android && -z "$device" ]] && { echo "--device SERIAL is required for android" >&2; exit 2; }

repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
short=$(git -C "$repo" rev-parse --short "$rev")
out="$evidence/$short"
export_dir="$evidence/export-$short-$platform"
mkdir -p "$out"
export LINGXIA_RUNNER_INSTANCE="${LINGXIA_RUNNER_INSTANCE:-readiness}"

echo "Exporting $short to $export_dir"
rm -rf "$export_dir"; mkdir -p "$export_dir"
git -C "$repo" archive "$short" | tar -x -C "$export_dir"
# Path dependencies outside the checkout (../fontique, ../../../../rong) resolve
# beside the export as they do beside the checkout.
for sibling in "$(dirname "$repo")"/*; do
  name=$(basename "$sibling")
  if grep -q "\.\./$name\"" "$repo/Cargo.toml" "$repo"/examples/*/native/Cargo.toml 2>/dev/null; then
    ln -sfn "$sibling" "$evidence/$name"
  fi
done

# shellcheck source=../lib/cargo-target-dir.sh
source "$repo/scripts/lib/cargo-target-dir.sh"
target=$(resolve_cargo_target_dir "$export_dir")
# The CLI embeds the bridge and polyfills from packages/, so those install first.
(cd "$export_dir/packages" && npm install --no-audit --no-fund >/dev/null)
(cd "$export_dir" && cargo build -p lingxia-cli -p lingxia-devtools-cli)
# Copied out at once: the target directory is shared, and another build may
# replace its binaries while this round runs.
mkdir -p "$export_dir/.bin"
cp "$target/debug/lingxia" "$target/debug/lxdev" "$export_dir/.bin/"
lingxia="$export_dir/.bin/lingxia"
lxdev="$export_dir/.bin/lxdev"
(cd "$export_dir" && "$lingxia" gen i18n >/dev/null)
# Generated SDK resources are not committed.
(cd "$export_dir" && "$lingxia" gen icons --input design/icons/svg \
  --android-out lingxia-sdk/android/lingxia/src/main/res/drawable >/dev/null)
(cd "$export_dir" && "$lingxia" gen icons --input design/icons/svg \
  --ios-out lingxia-sdk/apple/Sources/Resources/icons >/dev/null)
(cd "$export_dir/packages/lingxia-test" && npm run build >/dev/null)

# Every package an lxapp or the workspace resolves must be the export's own:
# a link back into a checkout would bundle its uncommitted code.
check_purity() {
  local root
  root=$(cd "$export_dir" && pwd -P)
  local link resolved bad=0
  while IFS= read -r link; do
    resolved=$(python3 -c 'import os, sys; print(os.path.realpath(sys.argv[1]))' "$link")
    case "$resolved" in
      "$root"/*) ;;
      *) echo "node_modules link $link resolves outside the export: $resolved" >&2; bad=1 ;;
    esac
  done < <(find "$export_dir/packages/node_modules" "$export_dir"/examples/*/lxapp/node_modules -maxdepth 2 -type l 2>/dev/null)
  (( bad == 0 )) || { echo "the export is not self-contained" >&2; exit 1; }
}

wake() {
  [[ "$platform" == android ]] || return 0
  adb -s "$device" shell input keyevent KEYCODE_WAKEUP
  adb -s "$device" shell wm dismiss-keyguard
  adb -s "$device" shell svc power stayon true
}

# The keyguard can come back mid-run even with stayon set. While a suite
# runs, a watchdog wakes and unlocks the phone within 3 s and records which
# case was running, so no failure is mistaken for a product defect.
anr_count() {
  adb -s "$device" shell dumpsys dropbox data_app_anr 2>/dev/null | grep -c ' data_app_anr ' || true
}

# The same watchdog notes each new ANR record (an "isn't responding" dialog
# holds focus and fails whatever runs under it) with the case it hit.
keyguard_watch() {
  local runner=$1 events=$2 log=$3 anrs=$4
  local seen
  seen=$(anr_count)
  while kill -0 "$runner" 2>/dev/null; do
    local now
    now=$(anr_count)
    if (( now > seen )); then
      local hit
      hit=$(grep '"case_started"' "$events" 2>/dev/null | tail -1 | sed -n 's/.*"id":"\([^"]*\)".*/\1/p')
      printf '{"at":"%s","case":"%s"}\n' "$(date -u +%FT%TZ)" "$hit" >>"$anrs"
      seen=$now
    fi
    local window power
    window=$(adb -s "$device" shell dumpsys window 2>/dev/null | grep -c 'isKeyguardShowing=true' || true)
    power=$(adb -s "$device" shell dumpsys power 2>/dev/null | grep -c 'mWakefulness=Asleep' || true)
    if (( window > 0 || power > 0 )); then
      local current
      current=$(grep '"case_started"' "$events" 2>/dev/null | tail -1 | sed -n 's/.*"id":"\([^"]*\)".*/\1/p')
      printf '{"at":"%s","case":"%s"}\n' "$(date -u +%FT%TZ)" "$current" >>"$log"
      wake
    fi
    sleep 3
  done
}

for app in $apps; do
  (cd "$export_dir/examples/$app/lxapp" && npm install --no-audit --no-fund >/dev/null)
done
check_purity

# Host-side device control for specs that turn the phone or read its native
# view tree: a capability URL reached from the phone through adb reverse.
fixture_args=()
fixture_pid=""
if [[ "$platform" == android ]]; then
  fixture_log="$out/device-fixture.log"
  fixture_port=$((20000 + RANDOM % 20000))
  node "$export_dir/examples/lingxia-showcase/lxapp/tests/harness/android-device-fixture.mjs" \
    --device "$device" --port "$fixture_port" >"$fixture_log" 2>&1 &
  fixture_pid=$!
  for _ in $(seq 1 50); do grep -q '^http' "$fixture_log" 2>/dev/null && break; sleep 0.1; done
  adb -s "$device" reverse "tcp:$fixture_port" "tcp:$fixture_port" >/dev/null
  fixture_args=(--secret-arg "androidDevice=$(head -1 "$fixture_log")")
fi
cleanup_fixture() {
  [[ -n "$fixture_pid" ]] || return 0
  kill "$fixture_pid" 2>/dev/null || true
  adb -s "$device" reverse --remove "tcp:$fixture_port" >/dev/null 2>&1 || true
  adb -s "$device" shell settings put system accelerometer_rotation 1 >/dev/null 2>&1 || true
}
trap cleanup_fixture EXIT

status=0
summary="["
for app in $apps; do
  project="$export_dir/examples/$app"
  result="$out/$app-$platform"
  mkdir -p "$result"
  session="readiness-$app"
  dev=(dev --background --platform "$platform" --name "$session")
  [[ -n "$device" ]] && dev+=(--device "$device")
  wake
  echo "Starting $app on $platform"
  if ! (cd "$project" && "$lingxia" "${dev[@]}") >"$result/dev.log" 2>&1; then
    echo "$app did not start; see $result/dev.log" >&2
    status=1
    summary+="{\"app\":\"$app\",\"started\":false},"
    continue
  fi
  test=(--session "$session" test --output-dir "$result/run" ${fixture_args[@]+"${fixture_args[@]}"})
  # The React showcase picks its platform entry by preset; the GPU one runs tests/.
  [[ "$app" == lingxia-showcase ]] && test+=(--preset "$platform")
  wake
  : >"$result/keyguard.jsonl"
  : >"$result/anr.jsonl"
  # A first launch syncs the dev bundle; restart once so the suite starts warm.
  if [[ "$platform" == android ]]; then
    (cd "$project" && "$lxdev" --session "$session" lxapp restart) >>"$result/dev.log" 2>&1 || true
    sleep 5
  fi
  set +e
  (cd "$project/lxapp" && "$lxdev" "${test[@]}") >"$result/test.log" 2>&1 &
  runner=$!
  watch=""
  if [[ "$platform" == android ]]; then
    keyguard_watch "$runner" "$result/run/events.jsonl" "$result/keyguard.jsonl" "$result/anr.jsonl" &
    watch=$!
  fi
  wait "$runner"
  code=$?
  # Only the watchdog: a bare wait would also wait for the device fixture.
  [[ -n "$watch" ]] && wait "$watch"
  set -e
  (cd "$project" && "$lxdev" --session "$session" logs --jsonl --limit 5000) >"$result/session.jsonl" 2>/dev/null || true
  (cd "$project" && "$lingxia" dev stop "$platform") >/dev/null 2>&1 || true
  report="$result/run/report.json"
  coverage=1 covered=false
  if [[ -f "$report" ]]; then
    kind=react; [[ "$app" == lingxia-showcase-gpu ]] && kind=gpu
    node "$export_dir/examples/lingxia-showcase/lxapp/tests/feature-coverage.mjs" \
      --report "$report" --app "$kind" >"$result/feature-coverage.txt" 2>&1 && coverage=0 covered=true
  fi
  (( code != 0 || coverage != 0 )) && status=1
  counts=$(node -e '
    const r = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"));
    const renderers = {};
    for (const c of r.cases ?? []) for (const name of c.renderers ?? []) renderers[name] = (renderers[name] ?? 0) + 1;
    const lines = (file) => require("fs").readFileSync(file, "utf8").split("\n").filter(Boolean).map((line) => JSON.parse(line));
    const locks = lines(process.argv[2]);
    const anrEvents = lines(process.argv[3]);
    const tainted = [...new Set([...locks, ...anrEvents].map((event) => event.case).filter(Boolean))];
    process.stdout.write(JSON.stringify({ passed: r.passed, failed: r.failed, skipped: r.skipped, timeout: r.timeout, total: (r.cases ?? []).length, renderers, keyguardEvents: locks.length, anrEvents, lockTainted: tainted }));
  ' "$report" "$result/keyguard.jsonl" "$result/anr.jsonl" 2>/dev/null || echo '{}')
  summary+="{\"app\":\"$app\",\"started\":true,\"exit\":$code,\"featureCoverage\":$covered,\"result\":$counts},"
  echo "$app: exit $code, $counts"
done
summary="${summary%,}]"
printf '{"rev":"%s","platform":"%s","device":"%s","apps":%s}\n' "$short" "$platform" "$device" "$summary" >"$out/summary.json"
echo "Summary: $out/summary.json"
(( keep )) || rm -rf "$export_dir"
exit $status
