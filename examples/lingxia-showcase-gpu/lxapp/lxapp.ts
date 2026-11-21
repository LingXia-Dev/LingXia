import {
  lx,
  type AppLaunchOptions,
  type ConfiguredPageName,
  type NavTarget,
  type OpenLink,
} from '@lingxia/logic';
import { appState, setIpAddr } from './shared/lib/app';
import { openFeedback } from './shared/lib/feedback';
import { getAppMessages } from './logic/app-messages';
import { resolveDisplayLanguage } from './shared/display-language';

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

/** Sidebar actions and the "More" menu, in the display language; re-run when it changes. */
async function applyHostChrome(os: string, tag: string) {
  const language = resolveDisplayLanguage(tag);
  const { t } = getAppMessages(language);
  type SidebarAction = Parameters<typeof lx.shell.sidebarActions.replace>[0][number];
  const sidebarActions: SidebarAction[] = [
    {
      id: 'downloads',
      placement: 'header',
      icon: 'public/sidebar-downloads.svg',
      label: t('sidebarDownloads'),
      onActivate: () => {
        void lx.shell.openBuiltin('downloads')
          .catch((error) => console.warn('downloads action failed', error));
      },
    },
    {
      id: 'chat',
      placement: 'footer',
      icon: 'public/sidebar-chat.svg',
      label: t('sidebarChat'),
      onActivate: () => {
        void lx.surface.openDeclared('lingxia-showcase-web')
          .catch((error) => console.warn('chat action failed', error));
      },
    },
  ];
  if (os === 'macOS' || os === 'Windows') {
    sidebarActions.push({
      id: 'terminal-settings',
      placement: 'footer',
      icon: 'public/sidebar-terminal-settings.svg',
      label: t('sidebarTerminalSettings'),
      onActivate: () => {
        void lx.surface.openApp('app.lingxia.terminal-settings', { as: 'aside', edge: 'right' })
          .catch((error) => console.warn('terminal settings action failed', error));
      },
    });
    sidebarActions.push({
      id: 'terminal',
      placement: 'footer',
      icon: 'public/sidebar-terminal.svg',
      label: t('sidebarTerminal'),
      onActivate: () => {
        void lx.surface.openDeclared('terminal')
          .catch((error) => console.warn('terminal action failed', error));
      },
    });
  }
  sidebarActions.push({
    id: 'ping',
    placement: 'footer',
    icon: 'public/activator.svg',
    label: t('sidebarPing'),
    onActivate: () => {
      lx.ui.toast(t('pingToast'), { tone: 'success' });
    },
  });
  // The one action with a `value`: the display language is a fact that changes, drawn beside
  // the icon.
  sidebarActions.push({
    id: 'language',
    placement: 'footer',
    icon: 'public/sidebar-language.svg',
    label: t('sidebarLanguage'),
    value: t('languageName'),
    onActivate: () => {
      const next = language === 'zh-CN' ? 'en-US' : 'zh-CN';
      void lx.host.displayLanguage.setPreference(next)
        .catch((error) => console.warn('language action failed', error));
    },
  });
  await lx.shell.sidebarActions.replace(sidebarActions);
  lx.app.setMoreActions([{
    icon: 'public/showcase-icon.png',
    label: t('moreFeedback'),
    onClick: async () => {
      try { await openFeedback(); }
      catch (error) { console.warn('failed to open feedback surface', error); }
    },
  }]);
}

App({
  // For `lxdev test`, which reads it through `appInstance().state`.
  state: appState,
  onLaunch(_options?: AppLaunchOptions) {
    appState.lifecycle.launches++;
    appState.lifecycle.last = 'launch';
    const { os } = lx.host.info;
    const applyChrome = (tag = lx.host.displayLanguage.get()) => {
      void applyHostChrome(os, tag)
        .catch((error) => console.warn('host chrome language update failed', error));
    };
    // A preference click always moves the preference event; the effective-tag watch alone can
    // miss a Harmony dispatch.
    lx.host.displayLanguage.watch((tag) => applyChrome(tag));
    lx.host.displayLanguage.watchPreference(() => applyChrome());

    lx.app.onUpdateReady(async () => {
      const { t } = getAppMessages(resolveDisplayLanguage(lx.host.displayLanguage.get()));
      const applyNow = await lx.ui.confirm({
        title: t('updateTitle'),
        message: t('updateBody'),
        cancelText: t('updateLater'),
        confirmText: t('updateApply'),
      });
      if (applyNow) lx.app.applyUpdate();
    });
    lx.app.onUpdateFailed((info) => {
      console.warn('Update failed', info);
    });
    // Not awaited: the first page's onLoad waits for onLaunch.
    void loadIpAddr();
  },
  onShow(options?: AppLaunchOptions) {
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
