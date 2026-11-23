// A project whose CLI-generated registry is present: names, tabs and each
// page's declared params and result reach lx.nav.
import { definePage, type } from '../../src/page.js';
import type { Lx } from '../../src/index.js';
import type { alpha, beta } from './cycle.js';

declare const lx: Lx;

const detail = definePage({
  params: type<{ id: number }>(),
  result: type<{ saved: boolean }>(),
  messages: type<{ in: { draft: string }; out: { typing: boolean } }>(),
});
const settings = definePage({});
const feed = definePage({
  params: type<{ item?: string }>(),
  setup: () => ({
    onShow(options) {
      const item: string | undefined = options.params?.item;
      // @ts-expect-error a show's params are the page's declared params
      const wrong: number | undefined = options.params?.item;
      void item; void wrong;
    },
  }),
});
const inbox = definePage({ params: type<{ thread: string }>() });

declare global {
  interface LxAppPages { home: true; detail: true; settings: true; alpha: true; beta: true; feed: true; inbox: true }
  interface LxAppTabs { home: true; feed: true; inbox: true }
  interface LxAppPageTypes {
    detail: typeof detail; settings: typeof settings; alpha: typeof alpha; beta: typeof beta;
    feed: typeof feed; inbox: typeof inbox;
  }
}

async function navigation(): Promise<boolean> {
  const handle = lx.nav.go({ page: 'detail', params: { id: 1 } });
  handle.send({ draft: 'hi' });
  handle.onMessage((message) => { const typing: boolean = message.typing; void typing; });
  // @ts-expect-error the opener sends the page's declared `in`
  handle.send({ typing: true });
  lx.nav.go({ page: 'settings' }).send({ anything: 1 });
  await handle.opened;
  const result = await handle.result;
  lx.nav.go({ page: 'settings' });
  await lx.nav.go({ tab: 'home' });
  await lx.nav.reset({ tab: 'home' });
  await lx.nav.go({ tab: 'feed', params: { item: '7' } });
  await lx.nav.go({ tab: 'feed' });
  await lx.nav.reset({ tab: 'feed', params: { item: '7' } });
  // @ts-expect-error a tab's params are checked against its page's declaration
  lx.nav.go({ tab: 'feed', params: { item: 7 } });
  // @ts-expect-error a tab page with a required param takes no params from a switch
  lx.nav.go({ tab: 'inbox', params: { thread: 'a' } });
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
