import { invoke, watch } from '../../shared/native';
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
  pattern?: string;
  action: string;
}

interface ProxySettings {
  mode: ProxyMode;
  socks5Host: string;
  socks5Port: number;
  username: string;
  password: string;
  autoSwitchRules: AutoSwitchRule[];
  gfwlistSourceUrl: string;
  isActive: boolean;
  statusMessage: string;
  gfwlistReady: boolean;
}

interface SettingsData {
  section: Section;
  appearance: string;
  language: string;
  download: DownloadSettings | null;
  usage: PrivacyUsage | null;
  /** The tab whose site the clear-site-data card targets, from the opening query. */
  siteTabId: string;
  site: { url: string; host: string } | null;
  proxy: ProxySettings | null;
  appInfo: AppInfo | null;
}

const SECTIONS: readonly Section[] = ['appearance', 'downloads', 'privacy', 'proxy', 'about'];

const initialData: SettingsData = {
  section: 'appearance',
  appearance: 'auto',
  language: 'auto',
  download: null,
  usage: null,
  siteTabId: '',
  site: null,
  proxy: null,
  appInfo: null,
};

export default definePage({
  data: initialData,
  // The browser opens Settings at a section, and for clear-site-data with the tab whose site it clears.
  params: type<{ section?: string; tabId?: string }>(),
  setup(page) {
    async function refreshUsage() {
      const usage = await invoke<PrivacyUsage>('privacy.getUsage');
      page.update((d) => { d.usage = usage; });
    }

    function applyDownload(download: DownloadSettings) {
      page.update((d) => { d.download = download; });
    }

    return {
      async onLoad(params) {
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
        void invoke<ProxySettings>('proxy.getSettings').then((proxy) => {
          page.update((d) => { d.proxy = proxy; });
          watch<ProxySettings>('proxy.watch', page.signal, (next) => {
            page.update((d) => { d.proxy = next; });
          });
        }, () => {});

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
        await refreshUsage().catch(() => {});
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
          applyDownload(await invoke<DownloadSettings>('downloads.chooseDirectory'));
        },
        async resetDownloadDir() {
          applyDownload(await invoke<DownloadSettings>('downloads.resetDirectory'));
        },
        async clearBrowsingData(input: ClearBrowsingDataInput): Promise<Notice> {
          const result = await invoke<{ historyRemoved: number }>('privacy.clearBrowsingData', input);
          await refreshUsage().catch(() => {});
          return { message: lx.app.t('privacy.cleared', { count: result.historyRemoved }) };
        },
        async clearSite({ cache, siteData }: { cache: boolean; siteData: boolean }): Promise<Notice | null> {
          const tabId = page.data.siteTabId;
          if (!tabId) return null;
          await invoke('privacy.clearSiteData', { tabId, cache, siteData });
          await refreshUsage().catch(() => {});
          return { message: lx.app.t('settings.privacy.status.siteCleared') };
        },
        async updateProxy({ mode, host, port }: { mode: ProxyMode; host: string; port: string }): Promise<Notice | null> {
          const current = page.data.proxy;
          if (!current) return null;
          const socks5Port = Number(port);
          if (!Number.isInteger(socks5Port) || socks5Port < 1 || socks5Port > 65535) {
            return { message: lx.app.t('settings.proxy.status.invalidPort'), tone: 'warning' };
          }
          // The route replaces every field, so send the ones this page does not edit unchanged.
          const next = await invoke<ProxySettings>('proxy.updateSettings', {
            mode,
            socks5Host: host.trim(),
            socks5Port,
            username: current.username,
            password: current.password,
            gfwlistSourceUrl: current.gfwlistSourceUrl,
            autoSwitchRules: current.autoSwitchRules,
          });
          page.update((d) => { d.proxy = next; });
          return { message: lx.app.t('settings.proxy.status.updated') };
        },
        async refreshGfwList(): Promise<Notice> {
          const next = await invoke<ProxySettings>('proxy.refreshGfwList');
          page.update((d) => { d.proxy = next; });
          return { message: lx.app.t('settings.proxy.status.rulesRefreshed') };
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
