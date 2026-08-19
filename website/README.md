# LingXia website

The marketing site for [LingXia](../Readme.md) — a polished, bilingual (EN / 中文)
landing page built with **Astro 5** and **Tailwind CSS v4**, ready for static hosting.

## Develop

```bash
cd website
npm install
npm run dev      # http://localhost:4321/LingXia/
```

| Command           | Action                                      |
| ----------------- | ------------------------------------------- |
| `npm run dev`     | Start the dev server with HMR               |
| `npm run build`   | Build the static site to `dist/`            |
| `npm run preview` | Serve the production build locally          |

## Structure

```text
website/
├─ astro.config.mjs        # site + base path (GitHub Pages)
├─ src/
│  ├─ pages/
│  │  ├─ index.astro       # English (served at /)
│  │  ├─ zh/index.astro    # 中文 (served at /zh/)
│  ├─ content/docs/        # Starlight guides (EN / 中文) + generated references
│  ├─ components/          # Landing sections + shared UI
│  │  └─ Landing.astro     # composes every section for a given language
│  ├─ layouts/Base.astro   # <head>, SEO/OG, fonts, ambient background
│  ├─ i18n/ui.ts           # ALL copy, keyed by language (en / zh)
│  ├─ lib/url.ts           # base-aware links + canonical URLs
│  └─ styles/global.css    # Tailwind v4 theme tokens + brand styles
└─ public/                 # favicon, logo, robots.txt
```

## Editing content

Landing-page copy lives in [`src/i18n/ui.ts`](src/i18n/ui.ts), keyed by language.
Edit the `en` and `zh` objects together — components read from `t = ui[lang]`.

Human guides live in `src/content/docs/guide/` and
`src/content/docs/zh/guide/`; keep those trees in parity. API pages are generated
from the pinned `@lingxia/types` package by `starlight-typedoc`. Component pages
are generated from `@lingxia/elements` by `scripts/gen-components.mjs`; edit the
generator or package JSDoc, not the generated Markdown.

Add a marketing language by extending `ui`, `LANGS`, and adding a
`src/pages/<lang>/index.astro` that renders `<Landing lang="<lang>" />`. A docs
locale also needs a Starlight locale entry and a matching content tree.

## Brand / design tokens

Colors, fonts, and effects are defined as Tailwind v4 `@theme` tokens in
[`src/styles/global.css`](src/styles/global.css):

- **Ink** — cool near-black canvas (`--color-ink-*`)
- **Jade** — primary accent, the "spark in the vessel" (`--color-jade-*`)
- **Cyan / Iris** — secondary aura
- Fonts: Space Grotesk (display) · Inter (body) · JetBrains Mono (code),
  all self-hosted via `@fontsource` (no external requests)

## Brand assets

The LingXia mark is a **jade isometric cube** — 匣 (vessel / module / gem), its
three faces echoing the View / Logic / Bridge layers, with a luminous core (灵).

| Asset | Where | Use |
| ----- | ----- | --- |
| Logo (vector) | `src/components/Logo.astro` | nav / footer mark + wordmark |
| Favicon | `public/favicon.svg` | browser tab |
| Social card | `public/og.png` (1200×630) | OpenGraph / Twitter |
| App icon (source) | `design/app-icon-{dark,light}.svg` | full-bleed 1024² for iOS/macOS/Android/Harmony icon sets |
| App icon (raster) | `public/app-icon.png` (1024²) | quick reference |
| Logo design rationale | `design/logo-concepts.html` | full brand-mark write-up (concept, construction, color, usage, explored directions) — `open` it in a browser |

App-icon SVGs are full-bleed squares; each platform applies its own corner mask,
so keep the cube within the centered safe area (already accounted for).

## Deployment

The site is a plain static build (`dist/`) — host it anywhere. Automated CI
publishing is intentionally **not** set up yet.

The production `lingxia.app` account transfer, Cloudflare Pages project,
custom domain, and rollback procedure are operational runbooks kept outside
this repository.

```bash
npm run build        # outputs static files to website/dist/
```

`dist/` can be served by any static host (Cloudflare Pages, Netlify, Vercel,
GitHub Pages, an S3 bucket, nginx, …). The build is configured for a **project
page** base path of `/LingXia` (`base: '/LingXia'` in `astro.config.mjs`), so it
expects to be served at `…/LingXia/`. Override per build with env vars:

```bash
SITE_URL=https://example.com BASE_PATH=/ npm run build
```

> When you're ready to automate GitHub Pages publishing, add a workflow using
> `actions/upload-pages-artifact` + `actions/deploy-pages`, and enable
> **repo Settings → Pages → Source: GitHub Actions**.

### Custom domain

To serve from e.g. `lingxia.dev`:

1. In `astro.config.mjs` set `site: 'https://lingxia.dev'` and `base: '/'`
   (or set the `SITE_URL` / `BASE_PATH` env vars in the workflow).
2. Add a `public/CNAME` file containing the domain.

Every internal link goes through `withBase()` in `src/lib/url.ts`, so changing
`base` is all that's needed — no per-link edits.
