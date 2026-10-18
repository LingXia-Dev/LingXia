#!/usr/bin/env bash
# Compile the launch storyboards into compiled/, which the CLI embeds and
# copies into every iOS app, so builds never need Interface Builder (ibtool
# needs the iOS platform installed). Each storyboard names only plain bundle
# images the CLI writes, so one compiled copy serves every app:
#   LingXiaSplashGround  solid ground, stretched to fill
#   LingXiaSplash        cover art, aspect-filled edge to edge
#   LingXiaLaunchMark    mark, centered at its @3x point size
# Needs Xcode with the iOS platform. Run after editing a .storyboard and
# commit compiled/; CI compiles them too and fails when compiled/ is stale.
set -euo pipefail
cd "$(dirname "$0")"
# The oldest iOS LingXia supports, so one compiled copy runs everywhere.
min_ios=17.0
rm -rf compiled
mkdir compiled
for source in *.storyboard; do
  name=${source%.storyboard}
  # ibtool may exit 0 on errors, or non-zero with them in its log.
  if ! log=$(xcrun ibtool --errors --warnings --target-device iphone --target-device ipad \
    --minimum-deployment-target "$min_ios" --output-format human-readable-text \
    --compile "compiled/$name.storyboardc" "$source" 2>&1) ||
    grep -q "com.apple.ibtool.errors" <<<"$log"; then
    echo "$source:" >&2
    echo "$log" >&2
    exit 1
  fi
done
