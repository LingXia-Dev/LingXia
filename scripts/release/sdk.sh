#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
LingXia SDK release packager (per-platform)

Generates SDK-required resources (i18n/icons), produces release artifacts,
and can optionally upload them to a GitHub Release via gh.

Usage:
  scripts/release/sdk.sh [--platform apple|ios|android|harmony|all] [--out <dir>]

Options:
  --platform <name>             apple/ios, Android, Harmony, or all (default: all)
  --out <dir>                   Output directory (default: dist/sdk-release)
  --no-shasums                  Skip SHASUMS file generation (useful for local dev)
  --android-maven-dir <dir>     Android: publish to this local Maven repo dir (default: <out>/android/maven)
  --android-no-zip              Android: do not create the release maven zip (useful for local dev)
  --apple-no-zip                Apple: do not create the source zip (useful for local dev)
  --gh-upload                   Upload generated artifacts to GitHub Release via gh CLI
  --tag <tag>                   Upload to this GitHub release tag (default: lingxia-sdk-v<version>)
  -h, --help                    Show help

Environment:
  SKIP_RUST=true                Skip swift-bridge refresh for Apple SDK
  GITHUB_TOKEN                  GitHub token used by gh (when --gh-upload is enabled)
  LINGXIA_RELEASE_REPO          Override GitHub repo (default: LingXia-Dev/LingXia)

Artifacts (under --out):
  lingxia-sdk-android-maven-<version>.zip
  lingxia-sdk-harmony-<version>.har
  lingxia-sdk-apple-source-<version>.zip
  SHASUMS256-<version>-<platforms>.txt
EOF
}

log() { echo "$*" >&2; }
die() { echo "❌ $*" >&2; exit 1; }

if [[ $# -eq 0 ]]; then
  usage
  exit 2
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
CALLER_DIR="$(pwd)"

to_abs_path() {
  local p="$1"
  if [[ "$p" == /* ]]; then
    printf '%s\n' "$p"
  else
    printf '%s\n' "$CALLER_DIR/$p"
  fi
}

VERSION=""
PLATFORM="all"
OUT_DIR="$ROOT_DIR/dist/sdk-release"
NO_SHASUMS=false
ANDROID_MAVEN_DIR=""
ANDROID_ZIP=true
APPLE_ZIP=true
GH_UPLOAD=false
GH_REPO="${LINGXIA_RELEASE_REPO:-LingXia-Dev/LingXia}"
GH_TAG=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --platform) PLATFORM="${2:-}"; shift 2 ;;
    --out) OUT_DIR="${2:-}"; shift 2 ;;
    --no-shasums) NO_SHASUMS=true; shift 1 ;;
    --android-maven-dir) ANDROID_MAVEN_DIR="${2:-}"; shift 2 ;;
    --android-no-zip) ANDROID_ZIP=false; shift 1 ;;
    --apple-no-zip) APPLE_ZIP=false; shift 1 ;;
    --gh-upload) GH_UPLOAD=true; shift 1 ;;
    --tag) GH_TAG="${2:-}"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) die "Unknown arg: $1 (try --help)" ;;
  esac
done

OUT_DIR="$(to_abs_path "$OUT_DIR")"
if [[ -n "$ANDROID_MAVEN_DIR" ]]; then
  ANDROID_MAVEN_DIR="$(to_abs_path "$ANDROID_MAVEN_DIR")"
fi
workspace_version="$(awk '
  /^\[workspace\.package\]/ {in_section=1; next}
  /^\[/ {in_section=0}
  in_section && $1 == "version" {
    gsub(/"/, "", $3);
    print $3;
    exit
  }' "$ROOT_DIR/Cargo.toml")"
[[ -n "$workspace_version" ]] || die "Failed to read workspace version from Cargo.toml"
VERSION="$workspace_version"
GH_TAG="${GH_TAG:-lingxia-sdk-v$VERSION}"

mkdir -p "$OUT_DIR"

WANT_ANDROID=false
WANT_APPLE=false
WANT_HARMONY=false

PLATFORM="$(echo "$PLATFORM" | tr '[:upper:]' '[:lower:]' | xargs)"

case "$PLATFORM" in
  all)
    WANT_APPLE=true
    WANT_ANDROID=true
    WANT_HARMONY=true
    platforms_slug="apple-android-harmony"
    ;;
  apple|ios)
    WANT_APPLE=true
    platforms_slug="apple"
    ;;
  android)
    WANT_ANDROID=true
    platforms_slug="android"
    ;;
  harmony)
    WANT_HARMONY=true
    platforms_slug="harmony"
    ;;
  *)
    die "Unknown platform: $PLATFORM (expected apple/ios, Android, Harmony, or all)"
    ;;
esac

I18N_DIR="$ROOT_DIR/i18n"
ICONS_SVG_DIR="$ROOT_DIR/lingxia-sdk/resources/icons/svg"
ANDROID_SDK_DIR="$ROOT_DIR/lingxia-sdk/android"
ANDROID_RES_DIR="$ANDROID_SDK_DIR/lingxia/src/main/res"
ANDROID_DRAWABLE_DIR="$ANDROID_RES_DIR/drawable"
LINGXIA_CRATES_DIR="$ROOT_DIR/crates"
LINGXIA_CORE_CRATE_DIR="$LINGXIA_CRATES_DIR/lingxia"
LINGXIA_WEBVIEW_CRATE_DIR="$LINGXIA_CRATES_DIR/lingxia-webview"
ANDROID_WEBVIEW_JAVA_SRC="$LINGXIA_WEBVIEW_CRATE_DIR/src/android/java"

APPLE_SDK_DIR="$ROOT_DIR/lingxia-sdk/apple"
APPLE_RES_DIR="$APPLE_SDK_DIR/Sources/Resources"
APPLE_ICONS_DIR="$APPLE_RES_DIR/icons"
APPLE_SOURCES_DIR="$APPLE_SDK_DIR/Sources"
APPLE_PACKAGE_SWIFT="$APPLE_SDK_DIR/Package.swift"
APPLE_STAGED_DIR="$ROOT_DIR/target/spm/lingxia"

HARMONY_SDK_DIR="$ROOT_DIR/lingxia-sdk/harmony"
HARMONY_RES_DIR="$HARMONY_SDK_DIR/lingxia/src/main/resources"
HARMONY_RAWFILE_DIR="$HARMONY_RES_DIR/rawfile"
HARMONY_ICONS_DIR="$HARMONY_RAWFILE_DIR/icons"
HARMONY_WEBVIEW_CORE_SRC="$LINGXIA_WEBVIEW_CRATE_DIR/src/harmony/arkts/WebViewCore.ets"
HARMONY_WEBVIEW_CORE_DST="$HARMONY_SDK_DIR/lingxia/src/main/ets/lxapp/WebViewCore.ets"

run() {
  log "+ $*"
  (cd "$ROOT_DIR" && "$@")
}

sync_harmony_webview_core_source() {
  [[ -f "$HARMONY_WEBVIEW_CORE_SRC" ]] || die "Missing Harmony WebView core source: $HARMONY_WEBVIEW_CORE_SRC"
  mkdir -p "$(dirname "$HARMONY_WEBVIEW_CORE_DST")"
  cp "$HARMONY_WEBVIEW_CORE_SRC" "$HARMONY_WEBVIEW_CORE_DST"
  log "   ✅ Synced WebView core ArkTS: $HARMONY_WEBVIEW_CORE_DST"
}

zip_dir() {
  local src_dir="$1"
  local out_zip="$2"
  local root_name="${3:-}"

  [[ -d "$src_dir" ]] || die "zip_dir source is not a directory: $src_dir"

  rm -f "$out_zip"
  mkdir -p "$(dirname "$out_zip")"

  if [[ -z "$root_name" ]]; then
    root_name="$(basename "$src_dir")"
  fi

  local tmp_dir
  tmp_dir="$(mktemp -d 2>/dev/null || mktemp -d -t lingxia_zip)"
  mkdir -p "$tmp_dir/$root_name"
  cp -R "$src_dir/." "$tmp_dir/$root_name/"
  find "$tmp_dir/$root_name" -name ".DS_Store" -delete 2>/dev/null || true
  (cd "$tmp_dir" && zip -qr "$out_zip" "$root_name" -x "*.DS_Store")
  rm -rf "$tmp_dir"
}

sha256_one() {
  local f="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$f" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$f" | awk '{print $1}'
  else
    die "Neither sha256sum nor shasum found"
  fi
}

write_shasums() {
  local out="$1"
  shift
  : > "$out"
  for f in "$@"; do
    local sum
    sum="$(sha256_one "$f")"
    [[ -n "$sum" ]] || die "Failed to compute sha256 for: $f"
    echo "$sum  $(basename "$f")" >> "$out"
  done
}

publish_github_release() {
  local tag="$1"
  shift
  local files=("$@")

  command -v gh >/dev/null 2>&1 || die "gh CLI not found. Install gh or remove --gh-upload."
  [[ ${#files[@]} -gt 0 ]] || die "No release assets to upload."

  for f in "${files[@]}"; do
    [[ -f "$f" ]] || die "Release asset not found: $f"
  done

  log "==> Publishing to GitHub Release ($GH_REPO @ $tag)"

  if gh release view "$tag" --repo "$GH_REPO" >/dev/null 2>&1; then
    log "   Release exists: $tag"
  else
    log "   Release missing, creating: $tag"
    local notes="LingXia SDK release ${VERSION}

- Tag: ${tag}
- Platform: ${PLATFORM}
- Generated by: scripts/release/sdk.sh
"
    gh release create "$tag" \
      --repo "$GH_REPO" \
      --title "LingXia SDK v$VERSION" \
      --notes "$notes"
  fi

  gh release upload "$tag" "${files[@]}" --repo "$GH_REPO" --clobber
  log "   ✅ Uploaded ${#files[@]} asset(s) to $GH_REPO:$tag"
}

generate_resources() {
  log "==> Generating SDK resources"
  [[ -d "$I18N_DIR" ]] || die "Missing i18n dir: $I18N_DIR"
  [[ -d "$ICONS_SVG_DIR" ]] || die "Missing icons svg dir: $ICONS_SVG_DIR"

  local gen_cmd=(cargo run -p lingxia-cli -- gen)
  local i18n_args=(i18n --input "$I18N_DIR" --no-rust --no-ts)
  local icons_args=(icons --input "$ICONS_SVG_DIR")

  if $WANT_ANDROID; then
    i18n_args+=(--android-out "$ANDROID_RES_DIR")
    icons_args+=(--android-out "$ANDROID_DRAWABLE_DIR")
  else
    i18n_args+=(--no-android)
  fi

  if $WANT_APPLE; then
    i18n_args+=(--ios-out "$APPLE_RES_DIR")
    icons_args+=(--ios-out "$APPLE_ICONS_DIR")
  else
    i18n_args+=(--no-ios)
  fi

  if $WANT_HARMONY; then
    i18n_args+=(--harmony-out "$HARMONY_RES_DIR")
    icons_args+=(--harmony-out "$HARMONY_ICONS_DIR")
  else
    i18n_args+=(--no-harmony)
  fi

  run "${gen_cmd[@]}" "${i18n_args[@]}"
  run "${gen_cmd[@]}" "${icons_args[@]}"

}

ensure_android_webview_sources() {
  [[ -d "$ANDROID_WEBVIEW_JAVA_SRC" ]] || die "Missing Android webview java src: $ANDROID_WEBVIEW_JAVA_SRC"
  log "==> Android WebView Java sources are compiled by Gradle sourceSets"
}

build_android() {
  log "==> Building Android SDK (maven zip)"
  ensure_android_webview_sources

  local maven_dir="${ANDROID_MAVEN_DIR:-}"
  if [[ -z "$maven_dir" ]]; then
    maven_dir="$OUT_DIR/android/maven"
  fi
  mkdir -p "$maven_dir"

  [[ -x "$ANDROID_SDK_DIR/gradlew" ]] || die "Missing gradlew: $ANDROID_SDK_DIR/gradlew"

  # Build Gradle properties
  local gradle_props=()
  gradle_props+=("-PLOCAL_MAVEN_REPO_DIR=$maven_dir")
  gradle_props+=("-Pversion=$VERSION")

  # Publish only to the local "localExample" Maven repository (a plain directory).
  # The Central Portal target (publishAndReleaseToMavenCentral) is deliberately
  # NOT invoked here — that runs in CI with credentials + a signing key.
  local publish_task=":lingxia:publishAllPublicationsToLocalExampleRepository"
  log "+ (cd $ANDROID_SDK_DIR && ./gradlew $publish_task ${gradle_props[*]})"
  (cd "$ANDROID_SDK_DIR" && ./gradlew "$publish_task" "${gradle_props[@]}" 1>&2)

  # groupId io.github.lingxia-dev -> io/github/lingxia-dev on disk.
  local aar="$maven_dir/io/github/lingxia-dev/lingxia/$VERSION/lingxia-$VERSION.aar"
  [[ -f "$aar" ]] || die "AAR not found after publish: $aar"

  if ! $ANDROID_ZIP; then
    log "   ✅ Android Maven repo ready: $maven_dir"
    printf '%s\n' "$maven_dir"
    return 0
  fi

  # For release assets, zip only this artifact's group subtree to avoid bundling unrelated local Maven contents.
  local group_dir="$maven_dir/io/github/lingxia-dev/lingxia"
  [[ -d "$group_dir" ]] || die "Android group dir missing: $group_dir"

  local tmp_dir
  tmp_dir="$(mktemp -d 2>/dev/null || mktemp -d -t lingxia_android_maven)"
  mkdir -p "$tmp_dir/maven/io/github/lingxia-dev"
  cp -R "$group_dir" "$tmp_dir/maven/io/github/lingxia-dev/"
  find "$tmp_dir/maven" -name ".DS_Store" -delete 2>/dev/null || true

  local out_zip="$OUT_DIR/lingxia-sdk-android-maven-$VERSION.zip"
  rm -f "$out_zip"
  (cd "$tmp_dir" && zip -qr "$out_zip" "maven" -x "*.DS_Store")
  rm -rf "$tmp_dir"

  log "   ✅ $out_zip"
  printf '%s\n' "$out_zip"
}

build_harmony() {
  log "==> Building HarmonyOS SDK (HAR)"
  [[ -d "$HARMONY_SDK_DIR" ]] || die "Missing Harmony SDK dir: $HARMONY_SDK_DIR"

  log "==> Syncing Harmony WebView core ArkTS"
  sync_harmony_webview_core_source

  rm -rf "$HARMONY_SDK_DIR/lingxia/oh_modules" 2>/dev/null || true
  rm -f "$HARMONY_SDK_DIR/lingxia/oh-package-lock.json5" 2>/dev/null || true

  log "==> Installing Harmony module dependencies"
  log "+ (cd $HARMONY_SDK_DIR/lingxia && ohpm install)"
  (cd "$HARMONY_SDK_DIR/lingxia" && ohpm install 1>&2)

  rm -f "$ROOT_DIR/target/ohpm/lingxia.har" 2>/dev/null || true
  rm -rf "$HARMONY_SDK_DIR/lingxia/build" 2>/dev/null || true

  log "+ (cd $HARMONY_SDK_DIR && hvigorw assembleHar)"
  (cd "$HARMONY_SDK_DIR" && hvigorw assembleHar 1>&2)

  local har
  har="$(find "$HARMONY_SDK_DIR/lingxia/build" -type f -name "*.har" | head -n1 || true)"
  [[ -n "$har" ]] || die "HAR not found under: $HARMONY_SDK_DIR/lingxia/build"

  # Also publish to workspace local repo used by example apps (ohpm file: dependency).
  mkdir -p "$ROOT_DIR/target/ohpm"
  cp "$har" "$ROOT_DIR/target/ohpm/lingxia.har"

  local out_har="$OUT_DIR/lingxia-sdk-harmony-$VERSION.har"
  cp "$har" "$out_har"
  log "   ✅ $out_har"

  printf '%s\n' "$out_har"
}

refresh_ios_generated() {
  if [[ "${SKIP_RUST:-}" == "true" ]]; then
    log "==> Skipping iOS Sources/generated refresh (SKIP_RUST=true)"
    return
  fi

  local gen_dir="$APPLE_SDK_DIR/Sources/generated"
  local sentinel="$gen_dir/SwiftBridgeCore.h"

  if [[ ! -f "$sentinel" ]]; then
    log "==> Refreshing iOS Sources/generated (missing)"
  else
    local needs=false

    if [[ -d "$LINGXIA_CORE_CRATE_DIR/src" ]] && find "$LINGXIA_CORE_CRATE_DIR/src" -type f -newer "$sentinel" | head -n 1 | grep -q .; then
      needs=true
    fi
    if [[ -f "$LINGXIA_CORE_CRATE_DIR/Cargo.toml" ]] && [[ "$LINGXIA_CORE_CRATE_DIR/Cargo.toml" -nt "$sentinel" ]]; then
      needs=true
    fi
    if [[ -f "$LINGXIA_CORE_CRATE_DIR/build.rs" ]] && [[ "$LINGXIA_CORE_CRATE_DIR/build.rs" -nt "$sentinel" ]]; then
      needs=true
    fi
    if [[ -f "$ROOT_DIR/Cargo.lock" ]] && [[ "$ROOT_DIR/Cargo.lock" -nt "$sentinel" ]]; then
      needs=true
    fi

    if ! $needs; then
      log "==> iOS Sources/generated up-to-date"
      return
    fi
    log "==> Refreshing iOS Sources/generated (inputs changed)"
  fi

  set +e
  (cd "$ROOT_DIR" && LINGXIA_GENERATE_BRIDGE=1 cargo build -p lingxia --target aarch64-apple-ios --release 1>&2)
  local rc=$?
  set -e
  if [[ $rc -ne 0 ]]; then
    die "Failed to refresh generated sources (swift-bridge). Fix the build errors, or ensure $APPLE_SDK_DIR/Sources/generated is up-to-date."
  fi
}

stage_ios_sdk() {
  log "==> Staging Apple SDK into target/ (for local dev + CLI parity)"
  rm -rf "$APPLE_STAGED_DIR" 2>/dev/null || true
  mkdir -p "$APPLE_STAGED_DIR"
  cp -R "$APPLE_SDK_DIR/." "$APPLE_STAGED_DIR/"
  rm -rf "$APPLE_STAGED_DIR/.build" "$APPLE_STAGED_DIR/.swiftpm" 2>/dev/null || true
  find "$APPLE_STAGED_DIR" -name ".DS_Store" -delete 2>/dev/null || true
}

build_ios_source() {
  log "==> Packaging Apple SDK (source zip, includes Sources/generated)"
  [[ -f "$APPLE_PACKAGE_SWIFT" ]] || die "Missing Package.swift: $APPLE_PACKAGE_SWIFT"
  [[ -d "$APPLE_SOURCES_DIR" ]] || die "Missing Sources/: $APPLE_SOURCES_DIR"

  refresh_ios_generated
  stage_ios_sdk

  if ! $APPLE_ZIP; then
    log "   ✅ Apple Sources ready: $APPLE_SDK_DIR"
    return 0
  fi

  local tmp_dir
  tmp_dir="$(mktemp -d 2>/dev/null || mktemp -d -t lingxia_ios_pkg)"
  local pkg_root="$tmp_dir/lingxia-apple-sdk"
  mkdir -p "$pkg_root"

  cp "$APPLE_PACKAGE_SWIFT" "$pkg_root/Package.swift"
  mkdir -p "$pkg_root/Sources"
  cp -R "$APPLE_SOURCES_DIR/." "$pkg_root/Sources/"
  find "$pkg_root" -name ".DS_Store" -delete 2>/dev/null || true

  local out_zip="$OUT_DIR/lingxia-sdk-apple-source-$VERSION.zip"
  rm -f "$out_zip"
  (cd "$tmp_dir" && zip -qr "$out_zip" "lingxia-apple-sdk" -x "*.DS_Store")
  rm -rf "$tmp_dir"

  log "   ✅ $out_zip"
  printf '%s\n' "$out_zip"
}

main() {
  generate_resources

  local artifacts=()

  if $WANT_ANDROID; then
    local android_out_file android_out
    android_out_file="$(mktemp 2>/dev/null || mktemp -t lingxia_android_out)"
    build_android >"$android_out_file"
    android_out="$(tail -n 1 "$android_out_file")"
    rm -f "$android_out_file"
    if [[ -n "$android_out" && -f "$android_out" ]]; then
      artifacts+=("$android_out")
    fi
  fi
  if $WANT_HARMONY; then
    local harmony_out_file harmony_out
    harmony_out_file="$(mktemp 2>/dev/null || mktemp -t lingxia_harmony_out)"
    build_harmony >"$harmony_out_file"
    harmony_out="$(tail -n 1 "$harmony_out_file")"
    rm -f "$harmony_out_file"
    if [[ -n "$harmony_out" && -f "$harmony_out" ]]; then
      artifacts+=("$harmony_out")
    fi
  fi
  if $WANT_APPLE; then
    local ios_out_file ios_out
    ios_out_file="$(mktemp 2>/dev/null || mktemp -t lingxia_apple_out)"
    build_ios_source >"$ios_out_file"
    ios_out="$(tail -n 1 "$ios_out_file")"
    rm -f "$ios_out_file"
    if [[ -n "$ios_out" && -f "$ios_out" ]]; then
      artifacts+=("$ios_out")
    fi
  fi

  if $GH_UPLOAD && [[ ${#artifacts[@]} -eq 0 ]]; then
    die "No SDK artifacts were generated for upload. Disable --android-no-zip/--apple-no-zip or change --platform."
  fi

  if ! $NO_SHASUMS; then
    local shasums="$OUT_DIR/SHASUMS256-$VERSION-$platforms_slug.txt"
    write_shasums "$shasums" "${artifacts[@]}"
    artifacts+=("$shasums")
    log "==> Checksums: $shasums"
  fi

  if $GH_UPLOAD; then
    publish_github_release "$GH_TAG" "${artifacts[@]}"
  fi
}

main
