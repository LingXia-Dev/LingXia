#!/bin/sh
# LingXia CLI installer (rustup / deno / bun style).
#
# Downloads a prebuilt `lingxia` binary from GitHub Releases, verifies its
# sha256 against the release SHASUMS file, and installs it to ~/.local/bin.
#
# macOS and Windows for now. Linux is not built yet.
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
# The installer runs from a POSIX shell. On Windows this means Git Bash, MSYS,
# or Cygwin; PowerShell users should run install.ps1 instead.
detect_os() {
  case "$(uname -s)" in
    Darwin) echo "darwin" ;;
    MINGW* | MSYS* | CYGWIN*) echo "windows" ;;
    Linux)
      err "Linux is not supported yet. Track progress and grab future builds at https://github.com/$REPO/releases"
      ;;
    *)
      err "unsupported OS '$(uname -s)'. Windows users: run this script from Git Bash/MSYS, or use install.ps1 in PowerShell"
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
if [ "$OS" = "windows" ]; then
  EXT=".exe"
else
  EXT=""
fi
# Binaries shipped together in the lingxia-cli release, installed as peers:
# the CLI (`lingxia`) and the devtools client (`lxdev`).
BINARIES="lingxia lxdev"

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

# The SHASUMS file covers every release asset; download it once and verify each
# binary against it.
info "Downloading checksums ..."
download "$BASE_URL/$SHASUMS" "$TMP_DIR/$SHASUMS" \
  || err "failed to download $SHASUMS from $BASE_URL"

# Pick the checksum tool once.
if command -v sha256sum >/dev/null 2>&1; then
  sha_check() { sha256sum -c "$1" >/dev/null; }
elif command -v shasum >/dev/null 2>&1; then
  sha_check() { shasum -a 256 -c "$1" >/dev/null; }
else
  err "need sha256sum or shasum to verify the download"
fi

mkdir -p "$INSTALL_DIR"

# Download, verify, and install each binary (the CLI and the devtools client).
for bin in $BINARIES; do
  asset="${bin}-${OS}-${ARCH}${EXT}"
  bin_name="${bin}${EXT}"

  info "Downloading $asset ..."
  download "$BASE_URL/$asset" "$TMP_DIR/$asset" \
    || err "failed to download $asset from $BASE_URL"

  # Isolate this asset's line so `-c` does not fail on files we did not download.
  expected_line="$(grep -E "[[:space:]]\*?${asset}\$" "$TMP_DIR/$SHASUMS" || true)"
  [ -n "$expected_line" ] || err "no checksum entry for $asset in $SHASUMS"
  echo "$expected_line" > "$TMP_DIR/$asset.sha256"
  ( cd "$TMP_DIR" && sha_check "$asset.sha256" ) \
    || err "checksum verification failed for $asset"

  dest="$INSTALL_DIR/$bin_name"
  mv "$TMP_DIR/$asset" "$dest"
  [ "$OS" = "windows" ] || chmod +x "$dest"
  info "Installed $bin_name -> $dest"
done

metadata_install_path="$INSTALL_DIR/lingxia${EXT}"
if [ "$OS" = "windows" ] && command -v cygpath >/dev/null 2>&1; then
  metadata_install_path="$(cygpath -w "$metadata_install_path")"
fi

json_escape() {
  printf '%s' "$1" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g'
}

metadata_repo="$(json_escape "$REPO")"
metadata_version="$(json_escape "$VERSION")"
metadata_install_path="$(json_escape "$metadata_install_path")"
cat > "$INSTALL_DIR/lingxia-cli-install.json" <<EOF
{
  "channel": "github-release",
  "repo": "$metadata_repo",
  "version": "$metadata_version",
  "install_path": "$metadata_install_path"
}
EOF
info "Installed update metadata -> $INSTALL_DIR/lingxia-cli-install.json"

info ""
info "Installed lingxia + lxdev $VERSION to $INSTALL_DIR; run \`lingxia${EXT} --version\`"

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
