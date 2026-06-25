#!/usr/bin/env bash
# Shared helper: always use a `lingxia` CLI built from THIS repo, not whatever
# stale binary happens to be on PATH. Source it, then `ensure_lingxia <root>`;
# afterwards $LINGXIA_BIN and PATH point at the freshly-built CLI. An explicit
# LINGXIA_BIN (anything but the default "lingxia") is honored as-is.
ensure_lingxia() {
  local root="$1" cargo="${CARGO_BIN:-cargo}"
  if [[ -n "${LINGXIA_BIN:-}" && "${LINGXIA_BIN}" != "lingxia" ]]; then
    return 0
  fi
  echo "==> Building lingxia CLI from source" >&2
  ( cd "$root" && "$cargo" build -q -p lingxia-cli )
  export LINGXIA_BIN="$root/target/debug/lingxia"
  export PATH="$root/target/debug:$PATH"
}
