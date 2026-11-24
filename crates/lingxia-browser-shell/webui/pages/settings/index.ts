import { invoke, watch } from '../../shared/native';
import { localDateTime } from '../../shared/format';
import { definePage, lx, type } from '@lingxia/logic';
import type { ClearBrowsingDataInput } from '../../shared/privacy';

type Section = 'appearance' | 'downloads' | 'privacy' | 'proxy' | 'about';
type ProxyMode = 'direct' | 'global' | 'gfw_list';
/** A toast's text and tone, translated here: the View's `t` works only while rendering. */
type Notice = { message: string; tone?: 'success' | 'warning' };

interface AppInfo {
  productName: string;
  version: string;
  sdkVersion: string;
  webuiVersion: string;
  gitSha: string;
}

interface DownloadSettings {
  downloadDir: string;
  usesDefaultDir: boolean;
  canChooseDirectory: boolean;
}

interface PrivacyUsage {
  historyEntries: number;
  cacheSites: number;
  siteDataSites: number;
  cookies: number;
}

interface AutoSwitchRule {
  pattern: string;
  action: 'proxy' | 'direct';
}

interface ProxySettings {
  mode: ProxyMode;
  socks5Host: string;
  socks5Port: number;
  username: string;
  password: string;
  autoSwitchRules: AutoSwitchRule[];
  status: string;
  localProxyAddr: string | null;
  gfwlistReady: boolean;
  gfwlistSourceUrl: string;
  gfwlistUpdatedAtMs: number | null;
  gfwlistStatus: string;
}

/** The proxy as the View shows it: the route's answer with its update time in local form. */
type ProxyView = Omit<ProxySettings, 'gfwlistUpdatedAtMs'> & { gfwlistUpdatedAt: string | null };

interface SettingsData {
  section: Section;
  appearance: string;
  language: string;
  download: DownloadSettings | null;
  usage: PrivacyUsage | null;
  /** False where the privacy routes do not exist; the section is then hidden. */
  hasPrivacy: boolean;
  /** The tab whose site the clear-site-data card targets, from the opening query. */
  siteTabId: string;
  site: { url: string; host: string } | null;
  proxy: ProxyView | null;
  /** The present, for "last updated". */
  now: string;
  appInfo: AppInfo | null;
}

const SECTIONS: readonly Section[] = ['appearance', 'downloads', 'privacy', 'proxy', 'about'];
const RULES_SETTLE_MS = 600;

const initialData: SettingsData = {
  section: 'appearance',
  appearance: 'auto',
  language: 'auto',
  download: null,
  usage: null,
  hasPrivacy: true,
  siteTabId: '',
  site: null,
  proxy: null,
  now: localDateTime(Date.now()),
  appInfo: null,
};

function proxyView(settings: ProxySettings): ProxyView {
  const { gfwlistUpdatedAtMs, ...rest } = settings;
  return { ...rest, gfwlistUpdatedAt: gfwlistUpdatedAtMs ? localDateTime(gfwlistUpdatedAtMs) : null };
}

/** A port typed by the user, or null when it is not one. */
function portOf(text: string): number | null {
  const port = Number(text.trim());
  return Number.isInteger(port) && port >= 1 && port <= 65535 ? port : null;
}

export default definePage({
  data: initialData,
  // The browser opens Settings at a section, and for clear-site-data with the tab whose site it clears.
  params: type<{ section?: string; tabId?: string }>(),
  setup(page) {
    async function refreshUsage() {
      try {
        const usage = await invoke<PrivacyUsage>('privacy.getUsage');
        page.update((d) => { d.usage = usage; });
      } catch {
        page.update((d) => { d.hasPrivacy = false; });
      }
    }

    function showProxy(settings: ProxySettings) {
      page.update((d) => {
        d.proxy = proxyView(settings);
        d.now = localDateTime(Date.now());
      });
    }

    // Proxy writes run one at a time, so a second tap waits for the first instead of racing it.
    let proxyQueue: Promise<unknown> = Promise.resolve();
    function serial<T>(work: () => Promise<T>): Promise<T> {
      const next = proxyQueue.then(work, work);
      proxyQueue = next.catch(() => {});
      return next;
    }

    /** Saves the proxy with `patch` over what is saved; the route replaces every field. */
    async function saveProxy(patch: Partial<ProxySettings>): Promise<ProxySettings> {
      const current = page.data.proxy;
      if (!current) throw new Error('proxy settings are not loaded');
      const saved = await invoke<ProxySettings>('proxy.updateSettings', {
        mode: current.mode,
        socks5Host: current.socks5Host,
        socks5Port: current.socks5Port,
        username: current.username,
        password: current.password,
        gfwlistSourceUrl: current.gfwlistSourceUrl,
        autoSwitchRules: current.autoSwitchRules,
        ...patch,
      });
      showProxy(saved);
      return saved;
    }

    async function downloadRules(): Promise<ProxySettings> {
      const next = await invoke<ProxySettings>('proxy.refreshGfwList');
      showProxy(next);
      return next;
    }

    // Rule edits settle before they are saved, so typing a pattern saves once.
    let rulesTimer: ReturnType<typeof setTimeout> | undefined;
    let rulesGeneration = 0;

    return {
      async onLoad(params) {
        // System pages open in their own stack; the bar shows back only when a page is under them.
        void page.navigationBar.update({ homeButton: 'hidden' });
        const requested = params?.section === 'clear-site-data' ? 'privacy' : params?.section;
        const tabId = typeof params?.tabId === 'string' ? params.tabId : '';
        page.update((d) => {
          if (SECTIONS.includes(requested as Section)) d.section = requested as Section;
          d.siteTabId = tabId;
        });

        watch<string>('app.watchAppearancePreference', page.signal, (pref) => {
          page.update((d) => { d.appearance = pref; });
        });
        watch<string>('app.watchDisplayLanguagePreference', page.signal, (pref) => {
          page.update((d) => { d.language = pref; });
        });
        // Proxy routes exist only where the local proxy runs; without them the section stays hidden.
        // The watch opens with the saved settings, then follows each change.
        watch<ProxySettings>('proxy.watch', page.signal, showProxy);

        const [info, appearance, language, download, site] = await Promise.all([
          invoke<AppInfo>('app.getInfo').catch(() => null),
          invoke<string>('app.getAppearancePreference').catch(() => null),
          invoke<string>('app.getDisplayLanguagePreference').catch(() => null),
          invoke<DownloadSettings>('downloads.getSettings').catch(() => null),
          tabId ? invoke<{ url: string; host: string }>('privacy.getSiteDataContext', { tabId }).catch(() => null) : null,
        ]);
        page.update((d) => {
          d.appInfo = info;
          if (appearance) d.appearance = appearance;
          if (language) d.language = language;
          d.download = download;
          d.site = site;
        });
        await refreshUsage();
      },
      actions: {
        async setSection({ section }: { section: Section }) {
          page.update((d) => { d.section = section; });
        },
        async setAppearance({ preference }: { preference: string }) {
          const next = await invoke<string>('app.setAppearancePreference', { preference });
          page.update((d) => { d.appearance = next; });
        },
        async setLanguage({ preference }: { preference: string }) {
          const next = await invoke<string>('app.setDisplayLanguagePreference', { preference });
          page.update((d) => { d.language = next; });
        },
        async chooseDownloadDir() {
          const download = await invoke<DownloadSettings>('downloads.chooseDirectory');
          page.update((d) => { d.download = download; });
        },
        async resetDownloadDir() {
          const download = await invoke<DownloadSettings>('downloads.resetDirectory');
          page.update((d) => { d.download = download; });
        },
        async clearBrowsingData(input: ClearBrowsingDataInput): Promise<Notice> {
          const result = await invoke<{ historyRemoved: number }>('privacy.clearBrowsingData', input);
          await refreshUsage();
          return { message: lx.app.t('privacy.cleared', { count: result.historyRemoved }) };
        },
        async clearSite({ cache, siteData }: { cache: boolean; siteData: boolean }): Promise<Notice | null> {
          const tabId = page.data.siteTabId;
          if (!tabId) return null;
          await invoke('privacy.clearSiteData', { tabId, cache, siteData });
          await refreshUsage();
          return { message: lx.app.t('settings.privacy.status.siteCleared') };
        },
        /** Switches routing over the saved server; Auto Switch without rules downloads them first. */
        async setProxyMode({ mode }: { mode: ProxyMode }): Promise<Notice> {
          return serial(async () => {
            const saved = await saveProxy({ mode });
            if (mode === 'gfw_list' && !saved.gfwlistReady) await downloadRules();
            return { message: lx.app.t('settings.proxy.status.modeSaved') };
          });
        },
        /** Saves the server fields; a field the route would refuse comes back as its own problem. */
        async saveProxyServer(input: { host: string; port: string; username: string; password: string }): Promise<{ field: 'host' | 'port'; message: string } | Notice> {
          const host = input.host.trim();
          const port = portOf(input.port);
          const mode = page.data.proxy?.mode ?? 'direct';
          if (!host && mode !== 'direct') return { field: 'host', message: lx.app.t('settings.proxy.status.hostRequired') };
          if (port === null) return { field: 'port', message: lx.app.t('settings.proxy.status.invalidPort') };
          return serial(async () => {
            await saveProxy({ socks5Host: host, socks5Port: port, username: input.username, password: input.password });
            return { message: lx.app.t('settings.proxy.status.serverSaved') };
          });
        },
        /**
         * Saves Auto Switch's own rules and list address once edits settle; a call that a later
         * edit replaced answers false without saving.
         */
        async saveAutoSwitch({ rules, url }: { rules: AutoSwitchRule[]; url: string }): Promise<boolean> {
          const generation = ++rulesGeneration;
          if (rulesTimer !== undefined) clearTimeout(rulesTimer);
          await new Promise<void>((resolve) => { rulesTimer = setTimeout(resolve, RULES_SETTLE_MS); });
          if (generation !== rulesGeneration) return false;
          const autoSwitchRules = rules
            .map((rule) => ({ pattern: rule.pattern.trim(), action: rule.action }))
            .filter((rule) => rule.pattern);
          await serial(() => saveProxy({ autoSwitchRules, gfwlistSourceUrl: url.trim() }));
          return true;
        },
        /** Downloads the rule list, saving a changed address first so the download uses it. */
        async refreshRules({ url }: { url: string }): Promise<Notice> {
          return serial(async () => {
            if (url.trim() && url.trim() !== page.data.proxy?.gfwlistSourceUrl) await saveProxy({ gfwlistSourceUrl: url.trim() });
            await downloadRules();
            return { message: lx.app.t('settings.proxy.status.rulesRefreshed') };
          });
        },
        async openHistory() {
          await lx.nav.go({ page: 'history' });
        },
        async openDownloads() {
          await lx.nav.go({ page: 'downloads' });
        },
      },
    };
  },
});
