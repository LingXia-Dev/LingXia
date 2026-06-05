#!/bin/sh
# LingXia CLI installer (rustup / deno / bun style).
#
# Downloads a prebuilt `lingxia` binary from GitHub Releases, verifies its
# sha256 against the release SHASUMS file, and installs it to ~/.local/bin.
#
# macOS only for now (arm64 + x86_64). Linux is not built yet; Windows users
# download the binary manually from the releases page.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/LingXia-Dev/LingXia/main/install.sh | sh
#
# Environment overrides:
#   LINGXIA_REPO         GitHub repo (default: LingXia-Dev/LingXia)
#   LINGXIA_VERSION      Version to install, e.g. 0.8.0 (default: latest CLI release)
#   LINGXIA_INSTALL_DIR  Install directory (default: $HOME/.local/bin)

set -eu

REPO="${LINGXIA_REPO:-LingXia-Dev/LingXia}"
INSTALL_DIR="${LINGXIA_INSTALL_DIR:-$HOME/.local/bin}"
TAG_PREFIX="lingxia-cli-v"

err() {
  echo "error: $*" >&2
  exit 1
}

info() {
  echo "$*"
}

# --- Pick a downloader -------------------------------------------------------
# Record the chosen tool once; fetch()/download() branch on it.
if command -v curl >/dev/null 2>&1; then
  DOWNLOADER="curl"
elif command -v wget >/dev/null 2>&1; then
  DOWNLOADER="wget"
else
  err "need curl or wget to download the CLI"
fi

# fetch <url> : print remote body to stdout (text/JSON).
fetch() {
  if [ "$DOWNLOADER" = "curl" ]; then
    curl -fsSL "$1"
  else
    wget -qO- "$1"
  fi
}

# download <url> <dest> : save remote body to a file.
download() {
  if [ "$DOWNLOADER" = "curl" ]; then
    curl -fsSL "$1" -o "$2"
  else
    wget -q "$1" -O "$2"
  fi
}

# --- Detect platform ---------------------------------------------------------
# The CLI currently ships macOS binaries only. Linux is not built yet; anything
# else (incl. Windows via uname/MSYS) gets the manual-download pointer.
detect_os() {
  case "$(uname -s)" in
    Darwin) echo "darwin" ;;
    Linux)
      err "Linux is not supported yet. Track progress and grab future builds at https://github.com/$REPO/releases"
      ;;
    *)
      err "unsupported OS '$(uname -s)'. Windows users: download the binary manually from https://github.com/$REPO/releases"
      ;;
  esac
}

detect_arch() {
  case "$(uname -m)" in
    x86_64 | amd64) echo "x86_64" ;;
    arm64 | aarch64) echo "aarch64" ;;
    *)
      err "unsupported architecture '$(uname -m)'"
      ;;
  esac
}

OS="$(detect_os)"
ARCH="$(detect_arch)"
# Asset name scheme matches .github/workflows/release-cli.yml exactly.
ASSET="lingxia-${OS}-${ARCH}"

# --- Resolve version ---------------------------------------------------------
# The repo ships several components, each with its own tag prefix (e.g.
# lingxia-cli-v*, sdk-v*), so /releases/latest is NOT reliable -- it returns the
# newest release of ANY component. Instead we list releases (newest-first) and
# take the first whose tag starts with "lingxia-cli-v".
resolve_version() {
  if [ -n "${LINGXIA_VERSION:-}" ]; then
    echo "$LINGXIA_VERSION"
    return
  fi
  tag="$(
    fetch "https://api.github.com/repos/$REPO/releases" \
      | grep -o '"tag_name"[[:space:]]*:[[:space:]]*"'"$TAG_PREFIX"'[^"]*"' \
      | head -n 1 \
      | sed -e 's/.*"'"$TAG_PREFIX"'/'"$TAG_PREFIX"'/' -e 's/"$//'
  )"
  [ -n "$tag" ] || err "could not find a $TAG_PREFIX release in $REPO"
  echo "${tag#"$TAG_PREFIX"}"
}

VERSION="$(resolve_version)"
TAG="${TAG_PREFIX}${VERSION}"
BASE_URL="https://github.com/$REPO/releases/download/$TAG"
SHASUMS="SHASUMS256-${VERSION}.txt"

info "Installing lingxia $VERSION ($OS/$ARCH) from $REPO"

# --- Download into a temp dir ------------------------------------------------
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT INT TERM

info "Downloading $ASSET ..."
download "$BASE_URL/$ASSET" "$TMP_DIR/$ASSET" \
  || err "failed to download $ASSET from $BASE_URL"

info "Downloading checksums ..."
download "$BASE_URL/$SHASUMS" "$TMP_DIR/$SHASUMS" \
  || err "failed to download $SHASUMS from $BASE_URL"

# --- Verify checksum ---------------------------------------------------------
# The SHASUMS file covers every release asset; isolate the line for our binary
# so `-c` does not fail on files we did not download.
info "Verifying checksum ..."
expected_line="$(grep -E "[[:space:]]${ASSET}\$" "$TMP_DIR/$SHASUMS" || true)"
[ -n "$expected_line" ] || err "no checksum entry for $ASSET in $SHASUMS"
echo "$expected_line" > "$TMP_DIR/$ASSET.sha256"

(
  cd "$TMP_DIR"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum -c "$ASSET.sha256" >/dev/null
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 -c "$ASSET.sha256" >/dev/null
  else
    err "need sha256sum or shasum to verify the download"
  fi
) || err "checksum verification failed for $ASSET"

# --- Install -----------------------------------------------------------------
mkdir -p "$INSTALL_DIR"
DEST="$INSTALL_DIR/lingxia"
mv "$TMP_DIR/$ASSET" "$DEST"
chmod +x "$DEST"

info ""
info "Installed lingxia $VERSION to $DEST; run \`lingxia --version\`"

# Warn if the install dir is not on PATH.
case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *)
    info ""
    info "note: $INSTALL_DIR is not on your PATH. Add it, e.g.:"
    info "  export PATH=\"$INSTALL_DIR:\$PATH\""
    info "Append that line to your shell profile (~/.bashrc, ~/.zshrc, ...) to persist it."
    ;;
esac
