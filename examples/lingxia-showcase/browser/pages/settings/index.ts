import { localDateTime } from '../../shared/format';
import { definePage, lx, type } from '@lingxia/logic';
import type {
  AppInfo,
  AutoSwitchRule,
  ClearBrowsingDataInput,
  DownloadSettingsResult,
  PrivacyUsage,
  ProxyMode,
  ProxySettingsInput,
  ProxySettingsResult,
  SiteDataContext,
} from '@lingxia/native';

type Section = 'appearance' | 'downloads' | 'privacy' | 'proxy' | 'about';
/** A toast's text and tone, translated here: the View's `t` works only while rendering. */
type Notice = { message: string; tone?: 'success' | 'warning' };
type AppearancePreference = Parameters<typeof lx.host.appearancePreference.set>[0];

/** An Auto Switch rule as the View edits it; the route leaves out an empty pattern and the default action. */
type ProxyRule = Required<AutoSwitchRule>;
/** The proxy as the View shows it: the route's answer with whole rules and its update time in local form. */
type ProxyView = Omit<ProxySettingsResult, 'gfwlistUpdatedAtMs' | 'autoSwitchRules'> & {
  autoSwitchRules: ProxyRule[];
  gfwlistUpdatedAt: string | null;
};
const APPEARANCES: readonly AppearancePreference[] = ['auto', 'light', 'dark'];

interface SettingsData {
  section: Section;
  appearance: string;
  language: string;
  download: DownloadSettingsResult | null;
  usage: PrivacyUsage | null;
  /** False where the privacy routes do not exist; the section is then hidden. */
  hasPrivacy: boolean;
  /** The tab whose site the clear-site-data card targets, from the opening query. */
  siteTabId: string;
  site: SiteDataContext | null;
  proxy: ProxyView | null;
  /** Why the last rule list download failed, translated; null once one succeeds. */
  listFailure: string | null;
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
  listFailure: null,
  now: localDateTime(Date.now()),
  appInfo: null,
};

function proxyView(settings: ProxySettingsResult): ProxyView {
  const { gfwlistUpdatedAtMs, autoSwitchRules, ...rest } = settings;
  return {
    ...rest,
    autoSwitchRules: autoSwitchRules.map((rule) => ({ pattern: rule.pattern ?? '', action: rule.action ?? 'proxy' })),
    gfwlistUpdatedAt: gfwlistUpdatedAtMs ? localDateTime(gfwlistUpdatedAtMs) : null,
  };
}

/** A failed rule list download in the user's words, from the route's `data.failure`; never the raw error. */
function listFailureText(error: unknown): string {
  const data = (error as { data?: { failure?: unknown; host?: unknown } } | null)?.data;
  const host = typeof data?.host === 'string' ? data.host : '';
  switch (data?.failure) {
    case 'proxyUnreachable': return lx.app.t('settings.proxy.list.why.proxyUnreachable', { host });
    case 'proxyRejected': return lx.app.t('settings.proxy.list.why.proxyRejected', { host });
    case 'sourceUnreachable': return lx.app.t('settings.proxy.list.why.sourceUnreachable', { host });
    case 'sourceTimedOut': return lx.app.t('settings.proxy.list.why.sourceTimedOut', { host });
    case 'notAList': return lx.app.t('settings.proxy.list.why.notAList', { host });
    case 'noProxyServer': return lx.app.t('settings.proxy.list.why.noProxyServer');
    case 'badAddress': return lx.app.t('settings.proxy.list.why.badAddress');
    default: return lx.app.t('settings.proxy.list.failedHint');
  }
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
        const usage: PrivacyUsage = await page.native.privacy.getUsage();
        page.update((d) => { d.usage = usage; });
      } catch {
        page.update((d) => { d.hasPrivacy = false; });
      }
    }

    function showProxy(settings: ProxySettingsResult) {
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
    async function saveProxy(patch: Partial<ProxySettingsInput>): Promise<ProxySettingsResult> {
      const current = page.data.proxy;
      if (!current) throw new Error('proxy settings are not loaded');
      const saved = await page.native.proxy.updateSettings({
        mode: current.mode,
        socks5Host: current.socks5Host,
        socks5Port: current.socks5Port,
        username: current.username,
        password: current.password,
        gfwlistSourceUrl: current.gfwlistSourceUrl,
        autoSwitchRules: [...current.autoSwitchRules],
        ...patch,
      });
      showProxy(saved);
      return saved;
    }

    /** Downloads the rule list; a failure is kept, translated, for the list card instead of thrown. */
    async function downloadRules(): Promise<boolean> {
      try {
        showProxy(await page.native.proxy.refreshGfwList());
        page.update((d) => { d.listFailure = null; });
        return true;
      } catch (error) {
        console.warn('proxy.refreshGfwList failed', error);
        page.update((d) => { d.listFailure = listFailureText(error); });
        return false;
      }
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

        // Each watch calls back with the current value at once.
        lx.host.appearancePreference.watch((pref) => {
          page.update((d) => { d.appearance = pref; });
        }, { signal: page.signal });
        lx.host.languagePreference.watch((pref) => {
          page.update((d) => { d.language = pref; });
        }, { signal: page.signal });
        // Proxy routes exist only where the local proxy runs; without them the section stays hidden.
        // The watch opens with the saved settings, then follows each change.
        page.native.proxy.watch(showProxy, { signal: page.signal });

        const [info, download, site]: [AppInfo | null, DownloadSettingsResult | null, SiteDataContext | null] = await Promise.all([
          page.native.app.getInfo().catch(() => null),
          page.native.downloads.getSettings().catch(() => null),
          tabId ? page.native.privacy.getSiteDataContext({ tabId }).catch(() => null) : null,
        ]);
        page.update((d) => {
          d.appInfo = info;
          d.download = download;
          d.site = site;
        });
        await refreshUsage();
      },
      actions: {
        async setSection({ section }: { section: Section }) {
          page.update((d) => { d.section = section; });
        },
        // The host refuses any other value; the watch above shows what was saved.
        async setAppearance({ preference }: { preference: string }) {
          const next = APPEARANCES.find((each) => each === preference);
          if (!next) return;
          await lx.host.appearancePreference.set(next);
        },
        async setLanguage({ preference }: { preference: string }) {
          await lx.host.languagePreference.set(preference);
        },
        async chooseDownloadDir() {
          const download = await page.native.downloads.chooseDirectory();
          page.update((d) => { d.download = download; });
        },
        async resetDownloadDir() {
          const download = await page.native.downloads.resetDirectory();
          page.update((d) => { d.download = download; });
        },
        async clearBrowsingData(input: ClearBrowsingDataInput): Promise<Notice> {
          const result = await page.native.privacy.clearBrowsingData(input);
          await refreshUsage();
          return { message: lx.app.t('privacy.cleared', { count: result.historyRemoved }) };
        },
        // Saves answer with no toast: the usage counts and the proxy's status badge show the result.
        async clearSite({ cache, siteData }: { cache: boolean; siteData: boolean }): Promise<void> {
          const tabId = page.data.siteTabId;
          if (!tabId) return;
          await page.native.privacy.clearSiteData({ tabId, cache, siteData });
          await refreshUsage();
        },
        /** Switches routing over the saved server; Auto Switch without rules downloads them first. */
        async setProxyMode({ mode }: { mode: ProxyMode }): Promise<void> {
          return serial(async () => {
            const saved = await saveProxy({ mode });
            if (mode === 'gfw_list' && !saved.gfwlistReady) await downloadRules();
          });
        },
        /** Saves the server fields; a field the route would refuse comes back as its own problem. */
        async saveProxyServer(input: { host: string; port: string; username: string; password: string }): Promise<{ field: 'host' | 'port'; message: string } | null> {
          const host = input.host.trim();
          const port = portOf(input.port);
          const mode = page.data.proxy?.mode ?? 'direct';
          if (!host && mode !== 'direct') return { field: 'host', message: lx.app.t('settings.proxy.status.hostRequired') };
          if (port === null) return { field: 'port', message: lx.app.t('settings.proxy.status.invalidPort') };
          return serial(async () => {
            await saveProxy({ socks5Host: host, socks5Port: port, username: input.username, password: input.password });
            return null;
          });
        },
        /**
         * Saves Auto Switch's own rules and list address once edits settle; a call that a later
         * edit replaced answers false without saving.
         */
        async saveAutoSwitch({ rules, url }: { rules: ProxyRule[]; url: string }): Promise<boolean> {
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
        /**
         * Downloads the rule list, saving a changed address first so the download uses it. The card shows
         * the result in place, so success needs no toast.
         */
        async refreshRules({ url }: { url: string }): Promise<void> {
          return serial(async () => {
            if (url.trim() && url.trim() !== page.data.proxy?.gfwlistSourceUrl) await saveProxy({ gfwlistSourceUrl: url.trim() });
            await downloadRules();
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
