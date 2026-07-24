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
| `icon-vessel-runner.svg` | Graphite tool tile with a `>_` prompt and offset vessel | Dev Runner identity (kept distinct from the light app icons) |
| `icon-vessel-glyph.svg` | Colored mark only, transparent | Android/Harmony layered foreground (`--foreground`) |
| `icon-vessel-macos.svg` | Squircle plate (824/1024 grid) + margin, transparent | Hand-built light macOS app art; the CLI normalizes full-bleed project sources itself |
| `icon-vessel-favicon.svg` | Full-canvas rounded plate, transparent corners | SDK `favicon.ico` (`lingxia://` tabs) |
| `appicon-*-1024.png` | 1024 renders of each master, including `appicon-runner-1024.png` | What the CLI / scripts actually consume |
| `icon-preview.html` | Visual check at 32–256 px, every mask/appearance | Open in a browser |

Regenerate the 1024 master PNGs after editing an SVG — the CLI renders them with
its own engine (resvg), no browser needed. The output format follows the
extension (`.png` here, `.ico` below); transparency is preserved automatically:

```sh
lingxia icon icon-vessel-light.svg   --output appicon-1024.png         --size 1024
lingxia icon icon-vessel-glyph.svg   --output appicon-glyph-1024.png   --size 1024
lingxia icon icon-vessel-macos.svg   --output appicon-macos-1024.png   --size 1024
lingxia icon icon-vessel-favicon.svg --output appicon-favicon-1024.png --size 1024
lingxia icon icon-vessel-runner.svg  --output appicon-runner-1024.png  --size 1024
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
| LingXia Runner (macOS) | `tools/lingxia-runner/macos/Sources/Resources/Assets.xcassets/AppIcon.appiconset/` | render `icon-vessel-runner.svg` at every size in the iconset |
| LingXia Runner (Windows) | `tools/lingxia-runner/windows/runner.ico` + `tools/lingxia-cli/assets/runner-icon.png` | `lingxia icon design/app-icon/icon-vessel-runner.svg --output …/runner.ico` and copy `appicon-runner-1024.png` to the CLI asset |
| Apple SDK browser-tab favicon | `lingxia-sdk/apple/Sources/Resources/favicon.ico` | `lingxia icon design/app-icon/icon-vessel-favicon.svg --output …/favicon.ico` |
| Browser shell webui (settings page) | `crates/lingxia-browser-shell/webui/public/LingXia.png` | copy `appicon-1024.png` |
| Website favicon / touch icon | `website/public/favicon.svg`, `website/public/app-icon.png` | favicon is hand-kept in sync; app-icon is a copy of `appicon-1024.png` |
| Website og:image | `website/public/og.png` | render `design/og/og-source.html` at 1200×630 |
| Showcase example | `examples/lingxia-showcase/AppIcon.png` + platform dirs | the `lingxia icon` commands above |
| Repo readme | `design/banner/banner.png` (mark + wordmark + tagline) | render `design/banner/banner-source.html` at 1200×340 @2x |
