# design/

Brand and visual design sources for LingXia. Everything here is a *master*:
the deployed copies (app icons, favicons, og images) are rendered from these
files — edit here, re-render, then update the consumers listed in each
subdirectory's README.

| Directory | Contents |
| --- | --- |
| `app-icon/` | The vessel mark: SVG masters (light/dark/glyph/macos/favicon), 1024 PNG renders, preview page, design rationale, deployment map |
| `banner/` | `banner-source.html` + `banner.png` — the readme banner, rendered at 1200×340 @2x |
| `og/` | `og-source.html` — the website og:image, rendered at 1200×630 with the site's self-hosted fonts |

Brand constants (shared with `website/src/styles/global.css`):

- Paper `#FAFAF7` · Ink `#1B1F26` (icon) / `#15181D` (dark plate)
- Jade `#14CF95` (pearl, light) / `#1FDDA4` (pearl, dark)
- Accent rule: jade leads, cyan supports, neutrals for everything else
