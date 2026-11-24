import { t, useAction, useLocal, type MessageKey } from '@lingxia/view';
import type { View } from './$types';

const choice = (selected: boolean) =>
  selected ? 'px-4 py-2 rounded-lg bg-accent text-on-accent text-sm' : 'px-4 py-2 rounded-lg bg-canvas border border-line text-sm';
const secondary = 'px-4 py-2 rounded-lg bg-canvas border border-line text-sm';
const danger = 'px-4 py-2 rounded-lg bg-danger-soft text-danger text-sm';
const card = 'p-4 rounded-xl bg-surface border border-line gap-3';

export function render({ data, actions }: View) {
  const proxyHost = useLocal('proxyHost', data.proxy?.socks5Host ?? '');
  const proxyPort = useLocal('proxyPort', data.proxy ? String(data.proxy.socks5Port) : '');
  const timeRange = useLocal<string>('timeRange', 'last24Hours');
  const clearHistory = useLocal<boolean>('clearHistory', true);
  const clearCache = useLocal<boolean>('clearCache', true);
  const clearSiteData = useLocal<boolean>('clearSiteData', false);
  const siteCache = useLocal<boolean>('siteCache', true);
  const siteData = useLocal<boolean>('siteData', true);

  const setSection = useAction(actions.setSection);
  const setAppearance = useAction(actions.setAppearance);
  const setLanguage = useAction(actions.setLanguage);
  const chooseDownloadDir = useAction(actions.chooseDownloadDir);
  const resetDownloadDir = useAction(actions.resetDownloadDir);
  const clearBrowsingData = useAction(actions.clearBrowsingData);
  const clearSite = useAction(actions.clearSite);
  const updateProxy = useAction(actions.updateProxy);
  const refreshGfwList = useAction(actions.refreshGfwList);
  const openHistory = useAction(actions.openHistory);
  const openDownloads = useAction(actions.openDownloads);

  const sections = (['appearance', 'downloads', 'privacy', 'proxy', 'about'] as const)
    .filter((id) => id !== 'proxy' || data.proxy);
  const host = proxyHost.value || data.proxy?.socks5Host || '';
  const port = proxyPort.value || (data.proxy ? String(data.proxy.socks5Port) : '');

  return (
    <page data-testid="settings-page" className="bg-canvas text-text px-6 py-6 gap-4">
      <view className="flex-row items-center justify-between">
        <text className="text-2xl font-semibold">{t('settings.title')}</text>
        {data.status ? <text className="text-xs text-accent">{t(data.status as MessageKey, { count: data.statusCount })}</text> : null}
      </view>

      <view className="flex-row gap-6 grow basis-0 min-h-0">
        <view className="w-48 gap-1">
          {sections.map((id) => (
            <button
              key={id}
              data-testid={`settings-nav-${id}`}
              onClick={() => { void setSection.run({ section: id }); }}
              className={data.section === id ? 'px-3 py-2 rounded-lg bg-accent text-on-accent text-sm' : 'px-3 py-2 rounded-lg bg-surface text-sm'}
            >
              <text>{t(`settings.${id}` as MessageKey)}</text>
            </button>
          ))}
        </view>

        <scroll className="grow basis-0 min-w-0 min-h-0">
          {data.section === 'appearance' ? (
            <view className="gap-6 max-w-xl">
              <view className={card}>
                <text className="text-base font-semibold">{t('settings.appearance')}</text>
                <view className="flex-row gap-2">
                  {(['auto', 'light', 'dark'] as const).map((pref) => (
                    <button key={pref} onClick={() => { void setAppearance.run({ preference: pref }); }} className={choice(data.appearance === pref)}>
                      <text>{t(`settings.appearance.${pref}` as MessageKey)}</text>
                    </button>
                  ))}
                </view>
              </view>
              <view className={card}>
                <text className="text-base font-semibold">{t('settings.language')}</text>
                <view className="flex-row gap-2">
                  {([['auto', 'settings.language.auto'], ['en-US', 'settings.language.en'], ['zh-CN', 'settings.language.zh']] as const).map(([id, label]) => (
                    <button key={id} onClick={() => { void setLanguage.run({ preference: id }); }} className={choice(data.language === id)}>
                      <text>{t(label)}</text>
                    </button>
                  ))}
                </view>
              </view>
            </view>
          ) : null}

          {data.section === 'downloads' ? (
            <view className={`${card} max-w-xl`}>
              <text className="text-base font-semibold">{t('settings.downloads.dir')}</text>
              <text className="text-sm text-muted">{data.download?.downloadDir ?? ''}</text>
              <view className="flex-row gap-2">
                {data.download?.canChooseDirectory ? (
                  <button onClick={() => { void chooseDownloadDir.run(); }} className="px-4 py-2 rounded-lg bg-accent text-on-accent text-sm">
                    <text>{t('settings.downloads.choose')}</text>
                  </button>
                ) : null}
                {data.download && !data.download.usesDefaultDir ? (
                  <button onClick={() => { void resetDownloadDir.run(); }} className={secondary}>
                    <text>{t('settings.downloads.reset')}</text>
                  </button>
                ) : null}
                <button onClick={() => { void openDownloads.run(); }} className={secondary}>
                  <text>{t('nav.downloads')}</text>
                </button>
              </view>
            </view>
          ) : null}

          {data.section === 'privacy' ? (
            <view className="gap-4 max-w-xl">
              {data.siteTabId ? (
                <view className={card}>
                  <text className="text-base font-semibold">{t('settings.privacy.site.title')}</text>
                  <text className="text-sm text-muted">{data.site?.host ?? ''}</text>
                  <checkbox bind={siteCache}><text className="text-sm">{t('settings.privacy.cache')}</text></checkbox>
                  <checkbox bind={siteData}><text className="text-sm">{t('settings.privacy.siteData')}</text></checkbox>
                  <view className="flex-row">
                    <button onClick={() => { void clearSite.run({ cache: siteCache.value, siteData: siteData.value }); }} className={danger}>
                      <text>{t('settings.privacy.clearCurrentSite')}</text>
                    </button>
                  </view>
                </view>
              ) : null}
              <view className={card}>
                <text className="text-base font-semibold">{t('settings.privacy.clearData')}</text>
                {data.usage ? (
                  <text className="text-sm text-muted">
                    {t('settings.privacy.usage', {
                      history: data.usage.historyEntries,
                      cacheSites: data.usage.cacheSites,
                      siteDataSites: data.usage.siteDataSites,
                      cookies: data.usage.cookies,
                    })}
                  </text>
                ) : null}
                <select
                  aria-label={t('settings.privacy.timeRange')}
                  bind={timeRange}
                  options={(['lastHour', 'last24Hours', 'last7Days', 'last4Weeks', 'allTime'] as const).map((value) => ({
                    value,
                    label: t(`settings.privacy.range.${value}` as MessageKey),
                  }))}
                />
                <checkbox bind={clearHistory}><text className="text-sm">{t('settings.privacy.history')}</text></checkbox>
                <checkbox bind={clearCache}><text className="text-sm">{t('settings.privacy.cache')}</text></checkbox>
                <checkbox bind={clearSiteData}><text className="text-sm">{t('settings.privacy.siteData')}</text></checkbox>
                <view className="flex-row flex-wrap gap-2">
                  <button
                    onClick={() => {
                      void clearBrowsingData.run({
                        timeRange: timeRange.value as 'lastHour' | 'last24Hours' | 'last7Days' | 'last4Weeks' | 'allTime',
                        history: clearHistory.value,
                        cache: clearCache.value,
                        siteData: clearSiteData.value,
                      });
                    }}
                    className={danger}
                  >
                    <text>{t('settings.privacy.clear')}</text>
                  </button>
                  <button onClick={() => { void openHistory.run(); }} className={secondary}>
                    <text>{t('settings.privacy.viewHistory')}</text>
                  </button>
                </view>
              </view>
            </view>
          ) : null}

          {data.section === 'proxy' && data.proxy ? (
            <view className={`${card} max-w-xl`}>
              <text className="text-base font-semibold">{t('settings.proxy')}</text>
              {data.proxy.statusMessage ? <text className="text-xs text-muted">{data.proxy.statusMessage}</text> : null}
              <view className="flex-row gap-2">
                {(['direct', 'global', 'gfw_list'] as const).map((mode) => (
                  <button key={mode} onClick={() => { void updateProxy.run({ mode, host, port }); }} className={choice(data.proxy?.mode === mode)}>
                    <text>{t(`settings.proxy.mode.${mode}` as MessageKey)}</text>
                  </button>
                ))}
              </view>
              <view className="flex-row gap-2">
                <input
                  aria-label={t('settings.proxy.host')}
                  value={host}
                  placeholder={t('settings.proxy.host')}
                  onInput={(v) => proxyHost.set(v)}
                  className="grow basis-0 px-3 py-2 rounded-lg bg-canvas border border-line text-sm"
                />
                <input
                  aria-label={t('settings.proxy.port')}
                  value={port}
                  placeholder={t('settings.proxy.port')}
                  inputMode="numeric"
                  onInput={(v) => proxyPort.set(v)}
                  className="w-28 px-3 py-2 rounded-lg bg-canvas border border-line text-sm"
                />
              </view>
              <view className="flex-row gap-2">
                <button
                  onClick={() => { if (data.proxy) void updateProxy.run({ mode: data.proxy.mode, host, port }); }}
                  className="px-4 py-2 rounded-lg bg-accent text-on-accent text-sm"
                >
                  <text>{t('settings.proxy.apply')}</text>
                </button>
                {data.proxy.mode === 'gfw_list' ? (
                  <button onClick={() => { void refreshGfwList.run(); }} className={secondary}>
                    <text>{t('settings.proxy.refreshRules')}</text>
                  </button>
                ) : null}
              </view>
            </view>
          ) : null}

          {data.section === 'about' && data.appInfo ? (
            <view className="p-4 rounded-xl bg-surface border border-line gap-2 max-w-xl">
              <text className="text-lg font-semibold">{data.appInfo.productName}</text>
              <text className="text-sm text-muted">{t('settings.about.version', { version: data.appInfo.version })}</text>
              <text className="text-xs text-muted">{t('settings.about.sdk', { version: data.appInfo.sdkVersion })}</text>
              <text className="text-xs text-muted">{t('settings.about.webui', { version: data.appInfo.webuiVersion })}</text>
              {data.appInfo.gitSha ? <text className="text-xs text-muted">{t('settings.about.commit', { sha: data.appInfo.gitSha })}</text> : null}
            </view>
          ) : null}
        </scroll>
      </view>
    </page>
  );
}
