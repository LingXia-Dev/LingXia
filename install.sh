#!/usr/bin/env sh
set -eu

REPO="${LINGXIA_REPO:-LingXia-Dev/LingXia}"
INSTALL_DIR="$HOME/.local/bin"
VERSION="0.7.0"
BIN_NAME="lingxia"
INSTALL_META_NAME="lingxia-cli-install.json"
RUNNER_ROOT_DIR="$HOME/.lingxia/runner"
RUNNER_APP_NAME="LingXia Runner.app"

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "error: missing required command: $1" >&2
    exit 1
  fi
}

json_read() {
  need_cmd python3
  script="$1"
  shift
  python3 -c "$script" "$@"
}

say() {
  printf '%s\n' "$*"
}

detect_platform() {
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Darwin) os="darwin" ;;
    Linux) os="linux" ;;
    *)
      echo "error: unsupported operating system: $os" >&2
      exit 1
      ;;
  esac

  case "$arch" in
    x86_64|amd64) arch="x64" ;;
    arm64|aarch64) arch="arm64" ;;
    *)
      echo "error: unsupported architecture: $arch" >&2
      exit 1
      ;;
  esac

  printf '%s-%s' "$os" "$arch"
}

download_file() {
  url="$1"
  output="$2"

  if command -v curl >/dev/null 2>&1; then
    if [ -n "${GITHUB_TOKEN:-}" ]; then
      curl -fsSL \
        -H "User-Agent: lingxia-install-script" \
        -H "Authorization: Bearer $GITHUB_TOKEN" \
        -o "$output" \
        "$url"
    else
      curl -fsSL \
        -H "User-Agent: lingxia-install-script" \
        -o "$output" \
        "$url"
    fi
    return
  fi

  if command -v wget >/dev/null 2>&1; then
    args=""
    if [ -n "${GITHUB_TOKEN:-}" ]; then
      args="--header=Authorization: Bearer $GITHUB_TOKEN"
    fi
    # shellcheck disable=SC2086
    wget -qO "$output" $args "$url"
    return
  fi

  echo "error: neither curl nor wget is available" >&2
  exit 1
}

github_api_get() {
  url="$1"

  if command -v curl >/dev/null 2>&1; then
    curl -fsSL \
      -H "User-Agent: lingxia-install-script" \
      -H "Accept: application/vnd.github+json" \
      -H "Authorization: Bearer $GITHUB_TOKEN" \
      "$url"
    return
  fi

  if command -v wget >/dev/null 2>&1; then
    wget -qO- \
      --header="User-Agent: lingxia-install-script" \
      --header="Accept: application/vnd.github+json" \
      --header="Authorization: Bearer $GITHUB_TOKEN" \
      "$url"
    return
  fi

  echo "error: neither curl nor wget is available" >&2
  exit 1
}

download_github_release_asset() {
  repo="$1"
  tag="$2"
  asset_name="$3"
  output="$4"

  release_api="https://api.github.com/repos/$repo/releases/tags/$tag"
  release_json="$(github_api_get "$release_api")"

  asset_url="$(
    printf '%s' "$release_json" | json_read '
import json, sys

asset_name = sys.argv[1]
release = json.load(sys.stdin)
for asset in release.get("assets", []):
    if asset.get("name") == asset_name:
        print(asset["url"])
        break
else:
    raise SystemExit(1)
' "$asset_name"
  )" || {
    echo "error: asset not found in release: $asset_name ($tag)" >&2
    exit 1
  }

  if command -v curl >/dev/null 2>&1; then
    curl -fsSL \
      -H "User-Agent: lingxia-install-script" \
      -H "Accept: application/octet-stream" \
      -H "Authorization: Bearer $GITHUB_TOKEN" \
      -o "$output" \
      "$asset_url"
    return
  fi

  if command -v wget >/dev/null 2>&1; then
    wget -qO "$output" \
      --header="User-Agent: lingxia-install-script" \
      --header="Accept: application/octet-stream" \
      --header="Authorization: Bearer $GITHUB_TOKEN" \
      "$asset_url"
    return
  fi

  echo "error: neither curl nor wget is available" >&2
  exit 1
}

ensure_install_dir() {
  mkdir -p "$INSTALL_DIR"
  if [ ! -w "$INSTALL_DIR" ]; then
    echo "error: install directory is not writable: $INSTALL_DIR" >&2
    exit 1
  fi
}

ensure_runner_dir() {
  mkdir -p "$1"
  if [ ! -w "$1" ]; then
    echo "error: runner install directory is not writable: $1" >&2
    exit 1
  fi
}

write_install_metadata() {
  meta_path="$INSTALL_DIR/$INSTALL_META_NAME"
  cat > "$meta_path" <<EOF
{
  "channel": "github-release",
  "repo": "$REPO",
  "version": "$version",
  "install_path": "$INSTALL_DIR/$BIN_NAME"
}
EOF
}

install_runner() {
  platform="$1"
  version="$2"

  case "$platform" in
    darwin-arm64) runner_arch="arm64" ;;
    darwin-x64) runner_arch="x64" ;;
    *) return 0 ;;
  esac

  runner_version_dir="$RUNNER_ROOT_DIR/$version"
  runner_app_path="$runner_version_dir/$RUNNER_APP_NAME"
  runner_asset_name="lingxia-runner-$version-macos-$runner_arch.zip"
  runner_download_url="https://github.com/$REPO/releases/download/lingxia-cli-v$version/$runner_asset_name"

  need_cmd unzip
  ensure_runner_dir "$runner_version_dir"

  say ""
  say "Installing LingXia Runner $version for $runner_arch"
  say "Download: $runner_download_url"

  temp_runner_zip="$temp_dir/$runner_asset_name"
  temp_runner_extract="$temp_dir/runner-extract"
  rm -rf "$temp_runner_extract"
  mkdir -p "$temp_runner_extract"

  if [ -n "${GITHUB_TOKEN:-}" ]; then
    download_github_release_asset "$REPO" "lingxia-cli-v$version" "$runner_asset_name" "$temp_runner_zip"
  else
    download_file "$runner_download_url" "$temp_runner_zip"
  fi

  unzip -q "$temp_runner_zip" -d "$temp_runner_extract"

  extracted_runner_app="$temp_runner_extract/$RUNNER_APP_NAME"
  if [ ! -d "$extracted_runner_app" ]; then
    echo "error: runner app bundle not found after unzip: $extracted_runner_app" >&2
    exit 1
  fi

  rm -rf "$runner_app_path"
  mv "$extracted_runner_app" "$runner_app_path"
  say "Installed Runner: $runner_app_path"
}

main() {
  need_cmd uname
  need_cmd mktemp
  need_cmd chmod
  need_cmd mkdir
  need_cmd mv

  if [ -n "${LINGXIA_INSTALL_DIR:-}" ] && [ "${LINGXIA_INSTALL_DIR}" != "$INSTALL_DIR" ]; then
    echo "error: custom install directories are no longer supported; LingXia CLI installs to $INSTALL_DIR" >&2
    exit 1
  fi

  platform="$(detect_platform)"
  version="$VERSION"
  tag="lingxia-cli-v$version"
  asset_name="lingxia-$platform"
  download_url="https://github.com/$REPO/releases/download/$tag/$asset_name"

  ensure_install_dir

  temp_dir="$(mktemp -d)"
  trap 'rm -rf "$temp_dir"' EXIT INT TERM
  temp_bin="$temp_dir/$BIN_NAME"

  say "Installing LingXia CLI $version for $platform"
  say "Download: $download_url"
  if [ -n "${GITHUB_TOKEN:-}" ]; then
    download_github_release_asset "$REPO" "$tag" "$asset_name" "$temp_bin"
  else
    download_file "$download_url" "$temp_bin"
  fi
  chmod +x "$temp_bin"
  mv "$temp_bin" "$INSTALL_DIR/$BIN_NAME"
  write_install_metadata
  install_runner "$platform" "$version"

  say ""
  say "Installed: $INSTALL_DIR/$BIN_NAME"

  case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *)
      say "Add this directory to your PATH if needed:"
      say "  export PATH=\"$INSTALL_DIR:\$PATH\""
      ;;
  esac

  say ""
  say "Verify:"
  say "  $BIN_NAME --version"
}

main "$@"
