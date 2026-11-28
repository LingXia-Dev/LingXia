import { definePage, lx, type PageSurface } from '@lingxia/logic';

const WINDOW_KEY = 'desktop-window';
const FLOAT_KEY = 'desktop-float';
const WEB_URL = 'https://cn.bing.com/';

export default definePage({
  data: {
    canWindow: false,
    canTab: false,
    /** What the opened page sent back with "Send then close", per placement. */
    windowMessage: '',
    floatMessage: '',
  },
  setup(page) {
    const offs: (() => void)[] = [];
    const stopListening = () => {
      for (const off of offs.splice(0)) off();
    };
    const run = async (open: () => Promise<unknown>) => {
      try {
        await open();
      } catch (error) {
        console.warn('[Desktop] open failed:', error);
        lx.ui.toast(lx.app.t('desktop.openFailed'));
      }
    };
    // A hidden page comes back as it was; a closed one opens anew under the same key.
    const openPage = async (key: string, as: 'window' | 'float') => {
      const existing = lx.surface.get({ key });
      if (existing?.kind === 'page' && existing.alive) return existing.show();
      const params = { from: 'desktop' };
      const surface: PageSurface = as === 'window'
        ? lx.surface.open({ page: 'surface', params }, {
            as: 'window',
            chrome: lx.supports('surface.window.fullChrome') ? 'full' : 'system',
            key,
            size: { width: 480, height: 640 },
          })
        : lx.surface.open({ page: 'surface', params }, { as: 'float', key, size: { width: '60%', height: '75%' } });
      offs.push(surface.onMessage((payload) => {
        const message = (payload as { message?: unknown } | null)?.message;
        page.update(d => {
          if (as === 'window') d.windowMessage = String(message ?? '');
          else d.floatMessage = String(message ?? '');
        });
      }));
      await surface.opened;
    };
    const actions = {
      openWindow: () => run(() => openPage(WINDOW_KEY, 'window')),
      openFloat: () => run(() => openPage(FLOAT_KEY, 'float')),
      openAside: () => run(() => lx.surface.open({ url: WEB_URL }, { as: 'aside', edge: 'right' })),
      openShowcaseWeb: () => run(() => lx.surface.open({ app: 'lingxia-showcase-web' }, { as: 'aside', edge: 'right' })),
      openTab: () => run(() => lx.surface.open({ url: WEB_URL }, { as: 'main' })),
      // The host declares the terminal for macOS and Windows, where this tab shows.
      openTerminal: () => run(() => lx.surface.open({ declared: 'terminal' })),
      // The product's scheme: every window, panel and aside follows it.
      switchAppearance: () => run(() => lx.host.appearancePreference.set(lx.app.appearance.get() === 'dark' ? 'light' : 'dark')),
    };
    return {
      onLoad() {
        page.update(d => {
          d.canWindow = lx.supports('surface.window');
          d.canTab = lx.supports('surface.tab');
        });
      },
      onUnload: stopListening,
      actions,
    };
  },
});
