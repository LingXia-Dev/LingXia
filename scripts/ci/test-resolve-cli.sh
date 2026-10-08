#!/usr/bin/env bash
# Exercise the real resolver and fingerprint in an isolated git repository.
set -euo pipefail
script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/repo/scripts/ci" "$work/repo/scripts/lib" "$work/bin"
cp "$script_dir/resolve-cli.sh" "$script_dir/cli-fingerprint.sh" "$work/repo/scripts/ci/"
cp "$script_dir/../lib/cargo-target-dir.sh" "$work/repo/scripts/lib/"

cat > "$work/bin/cargo" <<'CARGO'
#!/usr/bin/env bash
set -euo pipefail
[[ "$*" == 'build -p lingxia-cli -p lingxia-devtools-cli' ]] || exit 99
echo build >> "$BUILD_LOG"
[[ "${FAIL_BUILD:-0}" == 0 ]] || exit 101
mkdir -p "$CARGO_TARGET_DIR/debug"
for bin in lingxia lxdev; do
  printf '#!/usr/bin/env bash\nexit 0\n' > "$CARGO_TARGET_DIR/debug/$bin"
  chmod +x "$CARGO_TARGET_DIR/debug/$bin"
done
CARGO
# Skip network release lookup even on a developer's Mac.
printf '#!/usr/bin/env bash\necho Linux\n' > "$work/bin/uname"
chmod +x "$work/bin/"*
export PATH="$work/bin:$PATH" CARGO_TARGET_DIR="$work/build" BUILD_LOG="$work/build.log"
cd "$work/repo"
git init -q
git config user.name 'CI fixture'
git config user.email 'ci@example.invalid'
git config commit.gpgsign false
git config core.hooksPath /dev/null
mkdir -p design/icons/svg i18n/permission/cli
echo initial > Cargo.toml
git add .
git commit -qm initial

resolve() { bash scripts/ci/resolve-cli.sh --dest "$work/resolved" > "$work/output"; }
expect_source() { grep -qx "cli_source=$1" "$work/output"; }
expect_builds() { [[ $(wc -l < "$BUILD_LOG") -eq "$1" ]]; }
resolve
expect_source build
expect_builds 1
resolve
expect_source cache
expect_builds 1

# Changes to embedded inputs must invalidate an otherwise intact binary cache.
for input in Cargo.toml design/icons/svg/new.svg i18n/permission/cli/en-US.yaml; do
  before=$(wc -l < "$BUILD_LOG")
  echo changed >> "$input"
  git add "$input"
  git commit -qm "change $input"
  resolve
  expect_source build
  expect_builds "$((before + 1))"
done
rm "$work/resolved/lxdev"
resolve
expect_source build
expect_builds 5

stamp=$(cat "$work/resolved/.lingxia-cli-fingerprint")
echo changed-again >> Cargo.toml
git add Cargo.toml
git commit -qm changed-again
if FAIL_BUILD=1 resolve; then
  echo 'A failed build must not accept stale binaries' >&2
  exit 1
fi
[[ $(cat "$work/resolved/.lingxia-cli-fingerprint") == "$stamp" ]]
echo 'CLI resolver cache and fallback cases passed'
