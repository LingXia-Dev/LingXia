// @ts-check
import { defineConfig } from 'astro/config';
import tailwindcss from '@tailwindcss/vite';
import starlight from '@astrojs/starlight';

// The Logic JS API reference is generated into src/content/docs/reference/api/
// by scripts/gen-logic-api.mjs before astro runs (see package.json), one page
// per capability group. The sidebar picks it up as an ordinary directory.

// GitHub Pages project-site config.
// The repo is served at https://lingxia-dev.github.io/LingXia/, so `base` is the repo name.
// If you later attach a custom domain (e.g. lingxia.dev), set:
//   site: 'https://lingxia.dev', base: '/'
// and drop the trailing path from links — marketing pages go through `withBase()`
// in src/lib/url.ts, and Starlight handles base for the docs.
const SITE = process.env.SITE_URL ?? 'https://lingxia-dev.github.io';
const BASE = process.env.BASE_PATH ?? '/LingXia';
const GITHUB = 'https://github.com/LingXia-Dev/LingXia';

export default defineConfig({
  site: SITE,
  base: BASE,
  trailingSlash: 'ignore',
  build: {
    format: 'directory',
  },
  integrations: [
    // Docs live under /guide/* (en) and /zh/guide/* (zh). The bespoke marketing
    // pages (src/pages/index.astro, zh/index.astro) keep `/` and `/zh/`.
    starlight({
      title: 'LingXia',
      favicon: '/favicon.svg',
      customCss: ['./src/styles/starlight.css'],
      components: {
        FallbackContentNotice: './src/components/docs/FallbackContentNotice.astro',
      },
      social: [{ icon: 'github', label: 'GitHub', href: GITHUB }],
      defaultLocale: 'root',
      locales: {
        root: { label: 'English', lang: 'en' },
        zh: { label: '简体中文', lang: 'zh-CN' },
      },
      sidebar: [
        {
          label: 'Guide',
          translations: { 'zh-CN': '指南' },
          autogenerate: { directory: 'guide' },
        },
        {
          label: 'Reference',
          translations: { 'zh-CN': '参考' },
          items: [
            {
              slug: 'reference/logic-api',
              label: 'About the Logic API',
              translations: { 'zh-CN': '关于 Logic JS API' },
            },
            {
              label: 'Logic JS API',
              translations: { 'zh-CN': 'Logic JS API' },
              collapsed: true,
              autogenerate: { directory: 'reference/api' },
            },
            {
              label: 'Components',
              translations: { 'zh-CN': '组件' },
              autogenerate: { directory: 'reference/components' },
            },
          ],
        },
      ],
      editLink: {
        baseUrl: `${GITHUB}/edit/main/website/`,
      },
    }),
  ],
  vite: {
    plugins: [tailwindcss()],
  },
});
