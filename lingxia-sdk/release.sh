#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
LingXia SDK release packager (per-platform)

Generates SDK-required resources (i18n/assets/icons) and produces release artifacts
ready to be uploaded as GitHub Release assets by CI.

Usage:
  lingxia-sdk/release.sh --version <semver> [--platforms android,ios,harmony] [--out <dir>]

Options:
  --version <v>                 Version string used in artifact names (required)
  --platforms <csv>             android,ios,harmony (default: android,ios,harmony)
  --out <dir>                   Output directory (default: dist/sdk-release)
  --no-shasums                  Skip SHASUMS file generation (useful for local dev)
  --android-es5                 Android: build ES5 web runtime and publish version as <version>-es5
  --android-maven-dir <dir>     Android: publish to this local Maven repo dir (default: <out>/android/maven)
  --android-no-zip              Android: do not create the release maven zip (useful for local dev)
  --ios-no-zip                  iOS: do not create the source zip (useful for local dev)
  -h, --help                    Show help

Artifacts (under --out):
  lingxia-sdk-android-maven-<version>.zip
  lingxia-sdk-harmony-<version>.har
  lingxia-sdk-ios-source-<version>.zip
  SHASUMS256-<version>-<platforms>.txt
EOF
}

log() { echo "$*" >&2; }
die() { echo "❌ $*" >&2; exit 1; }

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

VERSION=""
PLATFORMS="android,ios,harmony"
OUT_DIR="$ROOT_DIR/dist/sdk-release"
NO_SHASUMS=false
ANDROID_ES5=false
ANDROID_MAVEN_DIR=""
ANDROID_ZIP=true
IOS_ZIP=true
ANDROID_VERSION=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version) VERSION="${2:-}"; shift 2 ;;
    --platforms) PLATFORMS="${2:-}"; shift 2 ;;
    --out) OUT_DIR="${2:-}"; shift 2 ;;
    --no-shasums) NO_SHASUMS=true; shift 1 ;;
    --android-es5) ANDROID_ES5=true; shift 1 ;;
    --android-maven-dir) ANDROID_MAVEN_DIR="${2:-}"; shift 2 ;;
    --android-no-zip) ANDROID_ZIP=false; shift 1 ;;
    --ios-no-zip) IOS_ZIP=false; shift 1 ;;
    -h|--help) usage; exit 0 ;;
    *) die "Unknown arg: $1 (try --help)" ;;
  esac
done

[[ -n "$VERSION" ]] || die "--version is required"

mkdir -p "$OUT_DIR"

IFS=',' read -r -a PLATFORM_ARR <<< "$PLATFORMS"
WANT_ANDROID=false
WANT_IOS=false
WANT_HARMONY=false

platforms_slug=""
for p in "${PLATFORM_ARR[@]}"; do
  p="$(echo "$p" | tr '[:upper:]' '[:lower:]' | xargs)"
  [[ -n "$p" ]] || continue
  platforms_slug+="${platforms_slug:+-}${p}"
  case "$p" in
    android) WANT_ANDROID=true ;;
    ios) WANT_IOS=true ;;
    harmony) WANT_HARMONY=true ;;
    *) die "Unknown platform: $p (expected android,ios,harmony)" ;;
  esac
done

[[ -n "$platforms_slug" ]] || die "--platforms is empty"

I18N_DIR="$ROOT_DIR/i18n"
ICONS_SVG_DIR="$ROOT_DIR/lingxia-sdk/resources/icons/svg"
ASSETS_DIR="$ROOT_DIR/lingxia-sdk/resources/assets"
WEB_RUNTIME_DIST="$ROOT_DIR/lingxia-web-runtime/dist"

ANDROID_SDK_DIR="$ROOT_DIR/lingxia-sdk/android"
ANDROID_RES_DIR="$ANDROID_SDK_DIR/lingxia/src/main/res"
ANDROID_ASSETS_OUT="$ANDROID_SDK_DIR/lingxia/src/main/assets"
ANDROID_DRAWABLE_DIR="$ANDROID_RES_DIR/drawable"
ANDROID_WEBVIEW_JAR_DIR="$ANDROID_SDK_DIR/lingxia/build/generated/lingxia-webview"
ANDROID_WEBVIEW_JAR="$ANDROID_WEBVIEW_JAR_DIR/lingxia-webview.jar"
ANDROID_WEBVIEW_JAVA_SRC="$ROOT_DIR/lingxia-webview/src/android/java"

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

run() {
  log "+ $*"
  (cd "$ROOT_DIR" && "$@")
}

build_web_runtime() {
  local web_dir="$ROOT_DIR/lingxia-web-runtime"
  [[ -f "$web_dir/package.json" ]] || die "Missing lingxia-web-runtime/package.json: $web_dir/package.json"
  [[ -d "$web_dir/node_modules" ]] || die "Missing $web_dir/node_modules. Run: (cd lingxia-web-runtime && npm ci)"

  if $ANDROID_ES5; then
    log "==> Building web runtime (Android ES5)"
    (cd "$web_dir" && npm run build:es5 1>&2)
  else
    log "==> Building web runtime (modern)"
    (cd "$web_dir" && npm run build 1>&2)
  fi
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

generate_resources() {
  log "==> Generating SDK resources (lingxia-gen)"
  [[ -d "$I18N_DIR" ]] || die "Missing i18n dir: $I18N_DIR"
  [[ -d "$ICONS_SVG_DIR" ]] || die "Missing icons svg dir: $ICONS_SVG_DIR"

  local i18n_args=(cargo run -p lingxia-gen -- i18n --input "$I18N_DIR")
  local assets_args=(cargo run -p lingxia-gen -- assets --input "$ASSETS_DIR" --runtime-input "$WEB_RUNTIME_DIST")
  local icons_args=(cargo run -p lingxia-gen -- icons --input "$ICONS_SVG_DIR")

  if $WANT_ANDROID; then
    i18n_args+=(--android-out "$ANDROID_RES_DIR")
    assets_args+=(--android-out "$ANDROID_ASSETS_OUT")
    icons_args+=(--android-out "$ANDROID_DRAWABLE_DIR")
  fi

  if $WANT_IOS; then
    i18n_args+=(--ios-out "$APPLE_RES_DIR")
    assets_args+=(--ios-out "$APPLE_RES_DIR")
    icons_args+=(--ios-out "$APPLE_ICONS_DIR")
  fi

  if $WANT_HARMONY; then
    i18n_args+=(--harmony-out "$HARMONY_RES_DIR")
    assets_args+=(--harmony-out "$HARMONY_RAWFILE_DIR")
    icons_args+=(--harmony-out "$HARMONY_ICONS_DIR")
  fi

  run "${i18n_args[@]}"
  run "${assets_args[@]}"
  run "${icons_args[@]}"
}

ensure_android_webview_jar() {
  mkdir -p "$ANDROID_WEBVIEW_JAR_DIR"
  if [[ -f "$ANDROID_WEBVIEW_JAR" ]]; then
    return
  fi
  [[ -d "$ANDROID_WEBVIEW_JAVA_SRC" ]] || die "Missing Android webview java src: $ANDROID_WEBVIEW_JAVA_SRC"
  log "==> Building lingxia-webview.jar (Makefile)"
  (cd "$ANDROID_WEBVIEW_JAVA_SRC" && TARGET_DIR="$ANDROID_WEBVIEW_JAR_DIR" make 1>&2)
  [[ -f "$ANDROID_WEBVIEW_JAR" ]] || die "Failed to build: $ANDROID_WEBVIEW_JAR"
}

build_android() {
  log "==> Building Android SDK (maven zip)"
  ensure_android_webview_jar

  local maven_dir="${ANDROID_MAVEN_DIR:-}"
  if [[ -z "$maven_dir" ]]; then
    maven_dir="$OUT_DIR/android/maven"
  fi
  mkdir -p "$maven_dir"

  [[ -x "$ANDROID_SDK_DIR/gradlew" ]] || die "Missing gradlew: $ANDROID_SDK_DIR/gradlew"

  log "+ (cd $ANDROID_SDK_DIR && ./gradlew :lingxia:publish ...)"
  (cd "$ANDROID_SDK_DIR" && \
    LINGXIA_JAR_OUTPUT_DIR="$ANDROID_WEBVIEW_JAR_DIR" \
    ./gradlew :lingxia:publish -PLOCAL_MAVEN_REPO_DIR="$maven_dir" -Pversion="$ANDROID_VERSION" 1>&2)

  local aar="$maven_dir/com/lingxia/lingxia/$ANDROID_VERSION/lingxia-$ANDROID_VERSION.aar"
  [[ -f "$aar" ]] || die "AAR not found after publish: $aar"

  if ! $ANDROID_ZIP; then
    log "   ✅ Android Maven repo ready: $maven_dir"
    printf '%s\n' "$maven_dir"
    return 0
  fi

  # For release assets, zip only this artifact's group subtree to avoid bundling unrelated local Maven contents.
  local group_dir="$maven_dir/com/lingxia/lingxia"
  [[ -d "$group_dir" ]] || die "Android group dir missing: $group_dir"

  local tmp_dir
  tmp_dir="$(mktemp -d 2>/dev/null || mktemp -d -t lingxia_android_maven)"
  mkdir -p "$tmp_dir/maven/com/lingxia"
  cp -R "$group_dir" "$tmp_dir/maven/com/lingxia/"
  find "$tmp_dir/maven" -name ".DS_Store" -delete 2>/dev/null || true

  local out_zip="$OUT_DIR/lingxia-sdk-android-maven-$ANDROID_VERSION.zip"
  rm -f "$out_zip"
  (cd "$tmp_dir" && zip -qr "$out_zip" "maven" -x "*.DS_Store")
  rm -rf "$tmp_dir"

  log "   ✅ $out_zip"
  printf '%s\n' "$out_zip"
}

build_harmony() {
  log "==> Building HarmonyOS SDK (HAR)"
  [[ -d "$HARMONY_SDK_DIR" ]] || die "Missing Harmony SDK dir: $HARMONY_SDK_DIR"

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
  local gen_dir="$APPLE_SDK_DIR/Sources/generated"
  local sentinel="$gen_dir/SwiftBridgeCore.h"

  if [[ ! -f "$sentinel" ]]; then
    log "==> Refreshing iOS Sources/generated (missing)"
  else
    local needs=false

    if [[ -d "$ROOT_DIR/lingxia/src" ]] && find "$ROOT_DIR/lingxia/src" -type f -newer "$sentinel" | head -n 1 | grep -q .; then
      needs=true
    fi
    if [[ -f "$ROOT_DIR/lingxia/Cargo.toml" ]] && [[ "$ROOT_DIR/lingxia/Cargo.toml" -nt "$sentinel" ]]; then
      needs=true
    fi
    if [[ -f "$ROOT_DIR/lingxia/build.rs" ]] && [[ "$ROOT_DIR/lingxia/build.rs" -nt "$sentinel" ]]; then
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
  log "==> Packaging iOS SDK (source zip, includes Sources/generated)"
  [[ -f "$APPLE_PACKAGE_SWIFT" ]] || die "Missing Package.swift: $APPLE_PACKAGE_SWIFT"
  [[ -d "$APPLE_SOURCES_DIR" ]] || die "Missing Sources/: $APPLE_SOURCES_DIR"

  refresh_ios_generated
  stage_ios_sdk

  if ! $IOS_ZIP; then
    log "   ✅ iOS Sources ready: $APPLE_SDK_DIR"
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

  local out_zip="$OUT_DIR/lingxia-sdk-ios-source-$VERSION.zip"
  rm -f "$out_zip"
  (cd "$tmp_dir" && zip -qr "$out_zip" "lingxia-apple-sdk" -x "*.DS_Store")
  rm -rf "$tmp_dir"

  log "   ✅ $out_zip"
  printf '%s\n' "$out_zip"
}

main() {
  if $ANDROID_ES5 && ( $WANT_IOS || $WANT_HARMONY ); then
    die "--android-es5 can only be used with --platforms android (no directory split for web runtime dist/)"
  fi

  ANDROID_VERSION="$VERSION"
  if $ANDROID_ES5 && [[ "$ANDROID_VERSION" != *-es5 ]]; then
    ANDROID_VERSION="${ANDROID_VERSION}-es5"
  fi

  build_web_runtime
  generate_resources

  local artifacts=()

  if $WANT_ANDROID; then
    local android_out
    android_out="$(build_android)"
    if [[ -n "$android_out" && -f "$android_out" ]]; then
      artifacts+=("$android_out")
    fi
  fi
  if $WANT_HARMONY; then
    local harmony_out
    harmony_out="$(build_harmony)"
    if [[ -n "$harmony_out" && -f "$harmony_out" ]]; then
      artifacts+=("$harmony_out")
    fi
  fi
  if $WANT_IOS; then
    local ios_out
    ios_out="$(build_ios_source)"
    if [[ -n "$ios_out" && -f "$ios_out" ]]; then
      artifacts+=("$ios_out")
    fi
  fi

  if ! $NO_SHASUMS; then
    local shasums="$OUT_DIR/SHASUMS256-$VERSION-$platforms_slug.txt"
    write_shasums "$shasums" "${artifacts[@]}"
    log "==> Checksums: $shasums"
  fi
}

main
