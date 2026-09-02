// Base-aware URL helper.
// Astro does NOT auto-prepend `base` to plain href/src strings, so every internal
// link and local asset goes through this. Change `base` in astro.config.mjs (e.g.
// to '/' for a custom domain) and all links follow automatically.
const BASE = import.meta.env.BASE_URL; // e.g. "/LingXia/" or "/"

/** Prefix an absolute-from-root path with the configured base path. */
export function withBase(path = '/'): string {
  const b = BASE.endsWith('/') ? BASE.slice(0, -1) : BASE;
  const p = path.startsWith('/') ? path : `/${path}`;
  return `${b}${p}` || '/';
}

export const GITHUB_URL = 'https://github.com/LingXia-Dev/LingXia';
export const GITHUB_RAW =
  'https://raw.githubusercontent.com/LingXia-Dev/LingXia/main';
export const INSTALL_CMD = `curl -fsSL ${GITHUB_RAW}/install.sh | sh`;

/** Link to a repository document that is not part of the website guide. */
export function repositoryDocUrl(rel: string): string {
  return `${GITHUB_URL}/blob/main/${rel}`;
}

/** Base-aware route to a translated website guide page. */
export function guideUrl(lang: 'en' | 'zh', slug: string): string {
  return withBase(`${lang === 'zh' ? '/zh' : ''}/guide/${slug}`);
}
