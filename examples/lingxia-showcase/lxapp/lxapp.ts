import {
  lx,
  type AppLifecycleEventArgs,
  type ConfiguredPageName,
  type NavTarget,
  type OpenLink,
} from '@lingxia/logic';
import { appState, setIpAddr } from './shared/lib/app';
import { openFeedback } from './shared/lib/feedback';

const PRODUCT_LINK_PREFIX = '/showcase/';

/** Where an App Link goes: a product path this app recognises, or `/lxapp/open?page=`. */
function targetOfLink(link: OpenLink): NavTarget | undefined {
  const pathname = link.url ? new URL(link.url).pathname : '';
  const page = pathname.startsWith(PRODUCT_LINK_PREFIX)
    ? pathname.slice(PRODUCT_LINK_PREFIX.length)
    : link.query.page;
  if (!page) return undefined;
  const params = { ...link.query };
  delete params.page;
  // A link carries whatever the sender wrote; navigation rejects a page this lxapp lacks.
  return { page: page as ConfiguredPageName, params };
}

/** Sidebar actions and the "More" menu, in this lxapp's language; re-run when it changes. */
async function applyHostChrome(os: typeof lx.host.info.os) {
  const t = lx.app.t;
  const language = lx.app.language.get();
  type SidebarAction = Parameters<typeof lx.shell.sidebarActions.replace>[0][number];
  const sidebarActions: SidebarAction[] = [
    {
      id: 'downloads',
      placement: 'header',
      icon: 'public/sidebar-downloads.svg',
      label: t('app.sidebar.downloads'),
      onSelect: () => {
        void lx.shell.openBuiltin('downloads')
          .catch((error) => console.warn('downloads action failed', error));
      },
    },
    {
      id: 'chat',
      placement: 'footer',
      icon: 'public/sidebar-chat.svg',
      label: t('app.sidebar.chat'),
      onSelect: () => {
        void lx.surface.openDeclared('lingxia-showcase-web')
          .catch((error) => console.warn('chat action failed', error));
      },
    },
  ];
  if (os === 'macos' || os === 'windows') {
    sidebarActions.push({
      id: 'terminal-settings',
      placement: 'footer',
      icon: 'public/sidebar-terminal-settings.svg',
      label: t('app.sidebar.terminalSettings'),
      onSelect: () => {
        void lx.surface.openApp('app.lingxia.terminal-settings', { as: 'aside', edge: 'right' })
          .catch((error) => console.warn('terminal settings action failed', error));
      },
    });
    sidebarActions.push({
      id: 'terminal',
      placement: 'footer',
      icon: 'public/sidebar-terminal.svg',
      label: t('app.sidebar.terminal'),
      onSelect: () => {
        void lx.surface.openDeclared('terminal')
          .catch((error) => console.warn('terminal action failed', error));
      },
    });
  }
  sidebarActions.push({
    id: 'ping',
    placement: 'footer',
    icon: 'public/activator.svg',
    label: t('app.sidebar.ping'),
    onSelect: () => {
      lx.ui.toast(t('app.sidebar.pingToast'), { tone: 'success' });
    },
  });
  // The one action with a `value`: the display language is a fact that changes, drawn beside
  // the icon.
  sidebarActions.push({
    id: 'language',
    placement: 'footer',
    icon: 'public/sidebar-language.svg',
    label: t('app.sidebar.language'),
    value: t('app.sidebar.languageName'),
    onSelect: () => {
      const next = language.startsWith('zh') ? 'en-US' : 'zh-CN';
      void lx.host.displayLanguage?.set(next)
        .catch((error) => console.warn('language action failed', error));
    },
  });
  await lx.shell.sidebarActions.replace(sidebarActions);
  lx.app.moreActions.replace([{
    icon: 'public/showcase-icon.png',
    label: t('app.moreFeedback'),
    onSelect: async () => {
      try { await openFeedback(); }
      catch (error) { console.warn('failed to open feedback surface', error); }
    },
  }]);
}

App({
  // For `lxdev test`, which reads it through `appInstance().state`.
  state: appState,
  onLaunch() {
    appState.lifecycle.launches++;
    appState.lifecycle.last = 'launch';
    const { os } = lx.host.info;
    const applyChrome = () => {
      void applyHostChrome(os)
        .catch((error) => console.warn('host chrome language update failed', error));
    };
    // A preference click always moves the preference event; the language watch alone can
    // miss a Harmony dispatch.
    lx.app.language.watch(() => applyChrome());
    lx.host.displayLanguage?.watch(() => applyChrome());

    lx.app.update.onReady(async () => {
      const applyNow = await lx.ui.confirm({
        title: lx.app.t('app.update.title'),
        message: lx.app.t('app.update.body'),
        cancelText: lx.app.t('app.update.later'),
        confirmText: lx.app.t('app.update.apply'),
      });
      if (applyNow) lx.app.update.apply();
    });
    lx.app.update.onFailed((info) => {
      console.warn('Update failed', info);
    });
    // Not awaited: the first page's onLoad waits for onLaunch.
    void loadIpAddr();
  },
  onShow(options?: AppLifecycleEventArgs) {
    appState.lifecycle.shows++;
    appState.lifecycle.last = 'show';
    appState.lifecycle.lastShow = options;
  },
  onHide() {
    appState.lifecycle.hides++;
    appState.lifecycle.last = 'hide';
  },
  onOpen(link: OpenLink) {
    return targetOfLink(link);
  },
});

async function loadIpAddr(): Promise<void> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 10_000);
  try {
    const response = await fetch('https://api64.ipify.org?format=json', { signal: controller.signal });
    if (!response.ok) throw new Error('Network information unavailable');
    const data = await response.json() as { ip: string };
    setIpAddr(data.ip);
  } catch {
    setIpAddr('Network information unavailable');
  } finally {
    clearTimeout(timeout);
  }
}
