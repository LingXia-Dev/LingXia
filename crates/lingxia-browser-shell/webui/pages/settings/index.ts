import { browserNative } from '../../shared/native';
import { definePage, lx } from '@lingxia/logic';

interface AppInfo {
  appName: string;
  version: string;
  gitSha?: string;
}

interface DownloadSettings {
  directory: string;
  isDefault: boolean;
}

interface PrivacyUsage {
  cacheBytes: number;
  siteDataBytes: number;
}

interface SiteDataContext {
  origin: string;
  host: string;
}

interface SettingsData {
  section: 'appearance' | 'downloads' | 'privacy' | 'proxy' | 'about';
  appearance: string;
  language: string;
  downloadDir: string;
  downloadDirIsDefault: boolean;
  cacheBytes: number;
  siteDataBytes: number;
  siteContext: SiteDataContext | null;
  proxyAvailable: boolean;
  proxyMode: string;
  proxyHost: string;
  proxyPort: string;
  appInfo: AppInfo;
  statusMessage: string;
}

const initialData: SettingsData = {
  section: 'appearance',
  appearance: 'auto',
  language: 'auto',
  downloadDir: '',
  downloadDirIsDefault: true,
  cacheBytes: 0,
  siteDataBytes: 0,
  siteContext: null,
  proxyAvailable: false,
  proxyMode: 'direct',
  proxyHost: '127.0.0.1',
  proxyPort: '7890',
  appInfo: { appName: 'LingXia', version: '' },
  statusMessage: '',
};

export default definePage({
  data: initialData,
  setup(page) {
    const native = browserNative(page);
    let disposed = false;
    const unwatchers: Array<() => void> = [];

    return {
      async onLoad(query?: Record<string, unknown>) {
        if (query?.section === 'clear-site-data' || query?.section === 'privacy') {
          page.update((d) => { d.section = 'privacy'; });
        } else if (query?.section === 'downloads' || query?.section === 'proxy' || query?.section === 'about') {
          page.update((d) => { d.section = query.section as SettingsData['section']; });
        }

        const [info, appearance, language, dl, privacy, proxy] = await Promise.all([
          native.invoke<AppInfo>('app.getInfo').catch(() => null),
          native.invoke<string>('app.getAppearancePreference').catch(() => 'auto'),
          native.invoke<string>('app.getDisplayLanguagePreference').catch(() => 'auto'),
          native.invoke<DownloadSettings>('downloads.getSettings').catch(() => null),
          native.invoke<PrivacyUsage>('privacy.getUsage').catch(() => null),
          native.invoke<{ settings?: { mode?: string; host?: string; port?: number } }>('proxy.getSettings').catch(() => null),
        ]);

        let siteContext: SiteDataContext | null = null;
        if (typeof query?.tabId === 'string' && query.tabId) {
          siteContext = await native
            .invoke<SiteDataContext>('privacy.getSiteDataContext', { tabId: query.tabId })
            .catch(() => null);
        }

        if (disposed) return;
        page.update((d) => {
          if (info) d.appInfo = info;
          d.appearance = appearance ?? 'auto';
          d.language = language ?? 'auto';
          if (dl) {
            d.downloadDir = dl.directory ?? '';
            d.downloadDirIsDefault = !!dl.isDefault;
          }
          if (privacy) {
            d.cacheBytes = privacy.cacheBytes ?? 0;
            d.siteDataBytes = privacy.siteDataBytes ?? 0;
          }
          d.siteContext = siteContext;
          if (proxy?.settings) {
            d.proxyAvailable = true;
            d.proxyMode = proxy.settings.mode ?? 'direct';
            d.proxyHost = proxy.settings.host ?? '127.0.0.1';
            d.proxyPort = String(proxy.settings.port ?? 7890);
          }
        });

        try {
          const apStream = native.stream<string>('app.watchAppearancePreference');
          unwatchers.push(() => apStream.close());
          (async () => {
            for await (const pref of apStream) {
              if (disposed) break;
              page.update((d) => { d.appearance = pref; });
            }
          })().catch(() => {});
        } catch {}

        try {
          const langStream = native.stream<string>('app.watchDisplayLanguagePreference');
          unwatchers.push(() => langStream.close());
          (async () => {
            for await (const pref of langStream) {
              if (disposed) break;
              page.update((d) => { d.language = pref; });
            }
          })().catch(() => {});
        } catch {}
      },
      onUnload() {
        disposed = true;
        for (const fn of unwatchers) fn();
        unwatchers.length = 0;
      },
      actions: {
        setSection({ section }: { section: SettingsData['section'] }) {
          page.update((d) => { d.section = section; d.statusMessage = ''; });
        },
        async setAppearance({ preference }: { preference: string }) {
          const next = await native.invoke<string>('app.setAppearancePreference', { preference });
          page.update((d) => { d.appearance = next; });
        },
        async setLanguage({ preference }: { preference: string }) {
          const next = await native.invoke<string>('app.setDisplayLanguagePreference', { preference });
          page.update((d) => { d.language = next; });
        },
        async chooseDownloadDir() {
          const res = await native.invoke<DownloadSettings>('downloads.chooseDirectory');
          if (res) {
            page.update((d) => {
              d.downloadDir = res.directory ?? d.downloadDir;
              d.downloadDirIsDefault = !!res.isDefault;
            });
          }
        },
        async resetDownloadDir() {
          const res = await native.invoke<DownloadSettings>('downloads.resetDirectory');
          if (res) {
            page.update((d) => {
              d.downloadDir = res.directory ?? d.downloadDir;
              d.downloadDirIsDefault = !!res.isDefault;
            });
          }
        },
        async clearCache() {
          await native.invoke('privacy.clearCache');
          const usage = await native.invoke<PrivacyUsage>('privacy.getUsage').catch(() => null);
          page.update((d) => {
            if (usage) d.cacheBytes = usage.cacheBytes ?? 0;
            d.statusMessage = 'Cache cleared';
          });
        },
        async clearAllSiteData() {
          await native.invoke('privacy.clearAllSiteData');
          const usage = await native.invoke<PrivacyUsage>('privacy.getUsage').catch(() => null);
          page.update((d) => {
            if (usage) d.siteDataBytes = usage.siteDataBytes ?? 0;
            d.statusMessage = 'Site data cleared';
          });
        },
        async clearCurrentSite() {
          if (!page.data.siteContext?.origin) return;
          await native.invoke('privacy.clearSiteData', { origin: page.data.siteContext.origin });
          page.update((d) => { d.statusMessage = `Cleared ${d.siteContext?.host ?? ''}`; });
        },
        async updateProxy({ mode, host, port }: { mode: string; host: string; port: string }) {
          await native.invoke('proxy.updateSettings', {
            mode,
            host: host.trim(),
            port: Number(port) || 7890,
          });
          page.update((d) => {
            d.proxyMode = mode;
            d.proxyHost = host;
            d.proxyPort = port;
            d.statusMessage = 'Proxy updated';
          });
        },
        async refreshGfwList() {
          await native.invoke('proxy.refreshGfwList');
          page.update((d) => { d.statusMessage = 'Rules refreshed'; });
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
