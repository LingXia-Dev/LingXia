# LingXia app icon — the vessel mark

## 设计理念 · Design rationale

**灵匣 = a luminous vessel.** The mark is the character 匣 reduced to its open
vessel radical **凵**, holding a rising pearl — the 灵. One open container, one
spark of life inside it: exactly what the runtime does (the host is the vessel;
lxapps are the pearls it holds).

- **The vessel** is drawn as a single rounded stroke (`stroke-linecap: round`,
  stroke = 96/1024). Ink `#1B1F26` on paper `#FAFAF7` in the light variant;
  paper `#F4F5F2` on ink `#15181D` in dark.
- **The pearl** is always jade — `#14CF95` on light, `#1FDDA4` on dark — the
  same jade that anchors the website palette. It sits at the vessel's mouth,
  rising, not contained: the spark is being released, not stored.
- **One pearl, always.** Multi-pearl compositions (the "host holds many
  lxapps" story) live in marketing/illustration surfaces, never in the icon:
  at 16–32 px extra pearls collapse into noise and dilute the mark.
- No gradients, no bevels: two flat colors survive every mask, size, and
  appearance mode.

The same geometry (viewBox 100: vessel path + pearl cx 50 cy 41.5 r 10) is used
by the website logo (`website/src/components/Logo.astro`) and favicon, so the
brand renders identically from 16 px favicon to 1024 px App Store art.

## Files

| File | What | Use |
| --- | --- | --- |
| `icon-vessel-light.svg` | Full-bleed, paper bg | Master for `lingxia icon` — all platforms |
| `icon-vessel-dark.svg` | Full-bleed, ink bg | Dark UI / marketing contexts |
| `icon-vessel-glyph.svg` | Colored mark only, transparent | Android/Harmony layered foreground (`--foreground`) |
| `icon-vessel-macos.svg` | Squircle plate (824/1024 grid) + margin, transparent | Hand-built macOS art (Runner appiconset); the CLI normalizes full-bleed sources itself |
| `icon-vessel-favicon.svg` | Full-canvas rounded plate, transparent corners | SDK `favicon.ico` for `lingxia://` browser tabs |
| `appicon-*-1024.png` | 1024 renders of each master | What the CLI / scripts actually consume |
| `icon-preview.html` | Visual check at 32–256 px, every mask/appearance | Open in a browser |

Regenerate the PNGs after editing an SVG (headless Chrome, exact 1024):

```sh
CHROME="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
"$CHROME" --headless --force-device-scale-factor=1 --window-size=1024,1024 \
  --screenshot=appicon-1024.png icon-vessel-light.svg
# transparent masters (glyph / macos / favicon) additionally need:
#   --default-background-color=00000000
```

## Generating app icons in a project

One command per concern — `lingxia icon` handles all platform plumbing,
including macOS normalization (Dock-scale + rounded corners + margin):

```sh
lingxia icon design/app-icon/appicon-1024.png -b "#FAFAF7" \
  --foreground design/app-icon/appicon-glyph-1024.png
```

## Where the icon is deployed

| Consumer | Path | How to update |
| --- | --- | --- |
| `lingxia new` template | `tools/lingxia-cli/templates/AppIcon.png` | copy `appicon-1024.png` |
| LingXia Runner (macOS) | `tools/lingxia-runner/Sources/Resources/Assets.xcassets/AppIcon.appiconset/` | `sips -z <s> <s> appicon-macos-1024.png --out icon_<s>.png` per size |
| Apple SDK browser-tab favicon | `lingxia-sdk/apple/Sources/Resources/favicon.ico` | pack `appicon-favicon-1024.png` downscales into PNG-in-ICO (16/32/64/256) |
| Browser shell webui (settings page) | `crates/lingxia-browser-shell/webui/public/LingXia.png` | copy `appicon-1024.png` |
| Website favicon / touch icon | `website/public/favicon.svg`, `website/public/app-icon.png` | favicon is hand-kept in sync; app-icon is a copy of `appicon-1024.png` |
| Website og:image | `website/public/og.png` | render `design/og/og-source.html` at 1200×630 |
| Showcase example | `examples/lingxia-showcase/AppIcon.png` + platform dirs | the `lingxia icon` commands above |
| Repo readme | `design/banner/banner.png` (mark + wordmark + tagline) | render `design/banner/banner-source.html` at 1200×340 @2x |
