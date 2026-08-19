// @ts-check
import { defineConfig } from 'astro/config';
import tailwindcss from '@tailwindcss/vite';
import starlight from '@astrojs/starlight';
import { createStarlightTypeDocPlugin } from 'starlight-typedoc';

// Build-time JS API reference. TypeDoc reads the published `@lingxia/types`
// package and emits markdown into src/content/docs/reference/api/, so the docs
// always track the pinned package version. `apiSidebarGroup` is a placeholder
// replaced in-place by the generated tree (see the Reference sidebar group).
const [starlightTypeDoc, apiSidebarGroup] = createStarlightTypeDocPlugin();

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
      plugins: [
        starlightTypeDoc({
          // Entry point resolved from node_modules — the published package's
          // main d.ts re-exports every `lx.*` namespace plus the `Lx` interface.
          entryPoints: ['./node_modules/@lingxia/types/dist/index.d.ts'],
          tsconfig: './tsconfig.typedoc.json',
          output: 'reference/api',
          sidebar: { label: 'Logic JS API', collapsed: true },
          typeDoc: {
            readme: 'none',
            githubPages: false,
            // Drop the repeated "Defined in: …/index.d.ts:NNN" line under every
            // member (pure noise here, and the source links were broken anyway).
            disableSources: true,
            entryFileName: 'index',
            indexFormat: 'table',
            parametersFormat: 'table',
            propertiesFormat: 'table',
            enumMembersFormat: 'table',
            typeDeclarationFormat: 'table',
            useCodeBlocks: true,
            // Inline option-object shapes (and their fields in parameter tables)
            // so the reference reads top-to-bottom instead of hopping across a
            // separate page per option/result type.
            expandObjects: true,
            expandParameters: true,
            // Starlight renders its own page title + nav; drop typedoc's
            // duplicate markdown header/breadcrumbs.
            hidePageHeader: true,
            hideBreadcrumbs: true,
          },
        }),
      ],
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
            // Replaced at build time by the generated TypeDoc group.
            apiSidebarGroup,
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
