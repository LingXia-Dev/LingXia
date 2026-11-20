// A project whose CLI-generated registry is present: names, tabs and each
// page's declared params and result reach lx.nav.
import { definePage, type } from '../../src/page.js';
import type { Lx } from '../../src/index.js';
import type { alpha, beta } from './cycle.js';

declare const lx: Lx;

const detail = definePage({
  params: type<{ id: number }>(),
  result: type<{ saved: boolean }>(),
});
const settings = definePage({});

declare global {
  interface LxAppPages { home: true; detail: true; settings: true; alpha: true; beta: true }
  interface LxAppTabs { home: true }
  interface LxAppPageTypes { detail: typeof detail; settings: typeof settings; alpha: typeof alpha; beta: typeof beta }
}

async function navigation(): Promise<boolean> {
  const handle = lx.nav.go({ page: 'detail', params: { id: 1 } });
  await handle.opened;
  const result = await handle.result;
  lx.nav.go({ page: 'settings' });
  await lx.nav.go({ tab: 'home' });
  await lx.nav.reset({ tab: 'home' });
  // @ts-expect-error detail declares required params
  lx.nav.go({ page: 'detail' });
  // @ts-expect-error params are checked against the page's declaration
  lx.nav.go({ page: 'detail', params: { id: '1' } });
  // @ts-expect-error only tab roots are tabs
  lx.nav.go({ tab: 'settings' });
  // @ts-expect-error an unknown page
  lx.nav.go({ page: 'missing' });
  return result.status === 'ok' ? result.value.saved : false;
}

export type RegistryGate = typeof navigation;
