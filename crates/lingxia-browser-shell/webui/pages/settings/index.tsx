import { t, useAction, useLocal } from '@lingxia/view';
import { formatBytes } from '../../shared/format';
import type { View } from './$types';

export function render({ data, actions }: View) {
  const proxyHost = useLocal('proxyHost', data.proxyHost);
  const proxyPort = useLocal('proxyPort', data.proxyPort);

  const setSection = useAction(actions.setSection);
  const setAppearance = useAction(actions.setAppearance);
  const setLanguage = useAction(actions.setLanguage);
  const chooseDownloadDir = useAction(actions.chooseDownloadDir);
  const resetDownloadDir = useAction(actions.resetDownloadDir);
  const clearCache = useAction(actions.clearCache);
  const clearAllSiteData = useAction(actions.clearAllSiteData);
  const clearCurrentSite = useAction(actions.clearCurrentSite);
  const updateProxy = useAction(actions.updateProxy);
  const refreshGfwList = useAction(actions.refreshGfwList);
  const openHistory = useAction(actions.openHistory);
  const openDownloads = useAction(actions.openDownloads);

  const navItems: Array<{ id: typeof data.section; label: string }> = [
    { id: 'appearance', label: t('settings.appearance') },
    { id: 'downloads', label: t('settings.downloads') },
    { id: 'privacy', label: t('settings.privacy') },
    ...(data.proxyAvailable ? [{ id: 'proxy' as const, label: t('settings.proxy') }] : []),
    { id: 'about', label: t('settings.about') },
  ];

  return (
    <page data-testid="settings-page" className="bg-canvas text-text px-6 py-6 gap-4">
      <view className="flex-row items-center justify-between">
        <text className="text-2xl font-semibold">{t('settings.title')}</text>
        {data.statusMessage ? <text className="text-xs text-accent">{data.statusMessage}</text> : null}
      </view>

      <view className="flex-row gap-6 grow basis-0 min-h-0">
        <view className="w-48 gap-1">
          {navItems.map((item) => (
            <button
              key={item.id}
              data-testid={`settings-nav-${item.id}`}
              onClick={() => { void setSection.run({ section: item.id }); }}
              className={data.section === item.id ? 'px-3 py-2 rounded-lg bg-accent text-on-accent text-sm' : 'px-3 py-2 rounded-lg bg-surface text-sm'}
            >
              <text>{item.label}</text>
            </button>
          ))}
        </view>

        <scroll className="grow basis-0 min-w-0 min-h-0">
          {data.section === 'appearance' ? (
            <view className="gap-6 max-w-xl">
              <view className="p-4 rounded-xl bg-surface border border-line gap-3">
                <text className="text-base font-semibold">{t('settings.appearance')}</text>
                <view className="flex-row gap-2">
                  {(['auto', 'light', 'dark'] as const).map((pref) => (
                    <button
                      key={pref}
                      onClick={() => { void setAppearance.run({ preference: pref }); }}
                      className={data.appearance === pref ? 'px-4 py-2 rounded-lg bg-accent text-on-accent text-sm' : 'px-4 py-2 rounded-lg bg-canvas border border-line text-sm'}
                    >
                      <text>{t(`settings.appearance.${pref}`)}</text>
                    </button>
                  ))}
                </view>
              </view>

              <view className="p-4 rounded-xl bg-surface border border-line gap-3">
                <text className="text-base font-semibold">{t('settings.language')}</text>
                <view className="flex-row gap-2">
                  {[
                    { id: 'auto', label: t('settings.language.auto') },
                    { id: 'en-US', label: 'English' },
                    { id: 'zh-CN', label: '简体中文' },
                  ].map((opt) => (
                    <button
                      key={opt.id}
                      onClick={() => { void setLanguage.run({ preference: opt.id }); }}
                      className={data.language === opt.id ? 'px-4 py-2 rounded-lg bg-accent text-on-accent text-sm' : 'px-4 py-2 rounded-lg bg-canvas border border-line text-sm'}
                    >
                      <text>{opt.label}</text>
                    </button>
                  ))}
                </view>
              </view>
            </view>
          ) : null}

          {data.section === 'downloads' ? (
            <view className="p-4 rounded-xl bg-surface border border-line gap-4 max-w-xl">
              <text className="text-base font-semibold">{t('settings.downloads.dir')}</text>
              <text className="text-sm text-muted">{data.downloadDir}</text>
              <view className="flex-row gap-2">
                <button onClick={() => { void chooseDownloadDir.run(); }} className="px-4 py-2 rounded-lg bg-accent text-on-accent text-sm">
                  <text>{t('settings.downloads.choose')}</text>
                </button>
                {!data.downloadDirIsDefault ? (
                  <button onClick={() => { void resetDownloadDir.run(); }} className="px-4 py-2 rounded-lg bg-canvas border border-line text-sm">
                    <text>{t('settings.downloads.reset')}</text>
                  </button>
                ) : null}
                <button onClick={() => { void openDownloads.run(); }} className="px-4 py-2 rounded-lg bg-canvas border border-line text-sm">
                  <text>{t('nav.downloads')}</text>
                </button>
              </view>
            </view>
          ) : null}

          {data.section === 'privacy' ? (
            <view className="gap-4 max-w-xl">
              <view className="p-4 rounded-xl bg-surface border border-line gap-3">
                <text className="text-base font-semibold">{t('settings.privacy')}</text>
                <text className="text-sm text-muted">
                  {`Cache: ${formatBytes(data.cacheBytes)} · Site Data: ${formatBytes(data.siteDataBytes)}`}
                </text>
                <view className="flex-row flex-wrap gap-2">
                  <button onClick={() => { void clearCache.run(); }} className="px-4 py-2 rounded-lg bg-canvas border border-line text-sm">
                    <text>{t('settings.privacy.clearCache')}</text>
                  </button>
                  <button onClick={() => { void clearAllSiteData.run(); }} className="px-4 py-2 rounded-lg bg-danger-soft text-danger text-sm">
                    <text>{t('settings.privacy.clearSiteData')}</text>
                  </button>
                  <button onClick={() => { void openHistory.run(); }} className="px-4 py-2 rounded-lg bg-canvas border border-line text-sm">
                    <text>{t('settings.privacy.viewHistory')}</text>
                  </button>
                </view>
              </view>
              {data.siteContext ? (
                <view className="p-4 rounded-xl bg-surface border border-line gap-3">
                  <text className="text-sm font-medium">{data.siteContext.host}</text>
                  <button onClick={() => { void clearCurrentSite.run(); }} className="px-4 py-2 rounded-lg bg-danger-soft text-danger text-sm">
                    <text>{t('settings.privacy.clearCurrentSite')}</text>
                  </button>
                </view>
              ) : null}
            </view>
          ) : null}

          {data.section === 'proxy' && data.proxyAvailable ? (
            <view className="p-4 rounded-xl bg-surface border border-line gap-4 max-w-xl">
              <text className="text-base font-semibold">{t('settings.proxy')}</text>
              <view className="flex-row gap-2">
                {(['direct', 'system', 'fixed', 'auto'] as const).map((m) => (
                  <button
                    key={m}
                    onClick={() => { void updateProxy.run({ mode: m, host: proxyHost.value, port: proxyPort.value }); }}
                    className={data.proxyMode === m ? 'px-3 py-1.5 rounded-lg bg-accent text-on-accent text-sm' : 'px-3 py-1.5 rounded-lg bg-canvas border border-line text-sm'}
                  >
                    <text>{m}</text>
                  </button>
                ))}
              </view>
              <view className="flex-row gap-2">
                <input
                  aria-label="Proxy host"
                  value={proxyHost.value}
                  placeholder="127.0.0.1"
                  onInput={(v) => proxyHost.set(v)}
                  className="grow basis-0 px-3 py-2 rounded-lg bg-canvas border border-line text-sm"
                />
                <input
                  aria-label="Proxy port"
                  value={proxyPort.value}
                  placeholder="7890"
                  onInput={(v) => proxyPort.set(v)}
                  className="w-28 px-3 py-2 rounded-lg bg-canvas border border-line text-sm"
                />
              </view>
              <view className="flex-row gap-2">
                <button
                  onClick={() => { void updateProxy.run({ mode: data.proxyMode, host: proxyHost.value, port: proxyPort.value }); }}
                  className="px-4 py-2 rounded-lg bg-accent text-on-accent text-sm"
                >
                  <text>{t('settings.proxy.apply')}</text>
                </button>
                <button onClick={() => { void refreshGfwList.run(); }} className="px-4 py-2 rounded-lg bg-canvas border border-line text-sm">
                  <text>{t('settings.proxy.refreshRules')}</text>
                </button>
              </view>
            </view>
          ) : null}

          {data.section === 'about' ? (
            <view className="p-4 rounded-xl bg-surface border border-line gap-2 max-w-xl">
              <text className="text-lg font-semibold">{data.appInfo.appName}</text>
              <text className="text-sm text-muted">{`Version ${data.appInfo.version}`}</text>
              {data.appInfo.gitSha ? <text className="text-xs text-muted">{`Commit ${data.appInfo.gitSha}`}</text> : null}
            </view>
          ) : null}
        </scroll>
      </view>
    </page>
  );
}
