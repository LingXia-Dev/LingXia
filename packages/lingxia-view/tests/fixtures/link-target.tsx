import type { LinkTarget } from '../../src/index.js';

declare global {
  interface LxAppPages { home: true; order: true; about: true }
  interface LxAppTabs { home: true }
  interface LxAppPageTypes {
    order: { readonly __lxPage: { readonly params: { id: string }; readonly result: unknown; readonly messages: never } };
  }
}

export const ok: LinkTarget[] = [
  { page: 'order', params: { id: '7' } },
  { page: 'order', params: { id: '7' }, replace: true },
  { page: 'about' },
  { tab: 'home' },
  { back: 1 },
  { app: 'com.x', page: 'anything' },
];
// @ts-expect-error an unknown page
export const unknown: LinkTarget = { page: 'nope' };
// @ts-expect-error a tab root is a tab, not a page
export const tabAsPage: LinkTarget = { page: 'home' };
// @ts-expect-error `order` requires its params
export const missing: LinkTarget = { page: 'order' };
// @ts-expect-error `replace` applies to a page only
export const replaceTab: LinkTarget = { tab: 'home', replace: true };
export const link = <link to={{ page: 'order', params: { id: '1' } }}>Order</link>;
// @ts-expect-error a link's `to` is checked too
export const badLink = <link to={{ page: 'order', params: { nope: 1 } }}>Order</link>;
