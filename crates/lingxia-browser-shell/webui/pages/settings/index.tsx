import { t, useAction, useLocal, type Child, type MessageKey } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { Card } from '@liulidesign/core/Card';
import { CheckboxInput } from '@liulidesign/core/CheckboxInput';
import { MetadataList } from '@liulidesign/core/MetadataList';
import { NavList } from '@liulidesign/core/NavList';
import { SegmentedControl } from '@liulidesign/core/SegmentedControl';
import { TextInput } from '@liulidesign/core/TextInput';
import { ToastRegion, useToast } from '@liulidesign/core/Toast';
import { TopBar } from '@liulidesign/core/TopBar';
import { ClearDataDialog } from '../../shared/ClearDataDialog';
import type { View } from './$types';

// The wire value is the Rust enum's; the catalogue key is camelCase.
const PROXY_MODES = [
  ['direct', 'settings.proxy.mode.direct'],
  ['global', 'settings.proxy.mode.global'],
  ['gfw_list', 'settings.proxy.mode.gfwList'],
] as const;

type Notice = { message: string; count?: number; tone?: 'success' | 'warning' } | null;

export function render(view: View) {
  return (
    <page data-testid="settings-page" className="bg-surface text-text">
      <ToastRegion>
        <Settings {...view} />
      </ToastRegion>
    </page>
  );
}

/** One card of settings: a heading, a line on what it does, then its controls. */
function SettingsCard({ title, description, children, testId }: { title: string; description?: string; children: Child; testId?: string }) {
  return (
    <Card variant="outlined" padding="lg" data-testid={testId}>
      <view className="gap-1">
        <text role="heading" headingLevel={2} className="text-callout font-semibold">{title}</text>
        {description ? <text className="text-footnote text-muted">{description}</text> : null}
      </view>
      {children}
    </Card>
  );
}

function Settings({ data, actions }: View) {
  const proxyHost = useLocal('proxyHost', data.proxy?.socks5Host ?? '');
  const proxyPort = useLocal('proxyPort', data.proxy ? String(data.proxy.socks5Port) : '');
  const siteCache = useLocal('siteCache', true);
  const siteData = useLocal('siteData', true);
  const clearing = useLocal('clearing', false);

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
  const toast = useToast();

  // Actions answer with a notice for the toast: a catalogue key, its count and tone.
  const run = async (outcome: Promise<{ ok: true; value: Notice } | { ok: false }>) => {
    const result = await outcome;
    const notice = result.ok ? result.value : null;
    if (notice) toast.show({ message: t(notice.message as MessageKey, { count: notice.count ?? 0 }), tone: notice.tone ?? 'success', id: 'settings' });
  };

  const sections = (['appearance', 'downloads', 'privacy', 'proxy', 'about'] as const)
    .filter((id) => id !== 'proxy' || data.proxy);
  const host = proxyHost.value || data.proxy?.socks5Host || '';
  const port = proxyPort.value || (data.proxy ? String(data.proxy.socks5Port) : '');

  return (
    <view className="grow min-h-0">
      <TopBar heading={t('settings.title')} className="px-6" />
      <view className="grow min-h-0 flex-row bg-canvas">
        <view className="w-[240px] shrink-0 px-3 py-4 border-e border-text/8">
          <NavList
            label={t('settings.title')}
            sections={[{ id: 'sections', items: sections.map((id) => ({ value: id, label: t(`settings.${id}` as MessageKey) })) }]}
            value={data.section}
            onChange={(section) => { void setSection.run({ section: section as typeof data.section }); }}
            data-testid="settings-nav"
          />
        </view>

        <scroll className="grow basis-0 min-w-0">
          <view className="w-full max-w-[680px] px-8 py-6 gap-4">
            {data.section === 'appearance' ? (
              <>
                <SettingsCard title={t('settings.appearance')} description={t('settings.appearance.hint')} testId="settings-appearance">
                  <SegmentedControl
                    label={t('settings.appearance')}
                    value={data.appearance}
                    onChange={(preference) => { void setAppearance.run({ preference }); }}
                    items={(['auto', 'light', 'dark'] as const).map((value) => ({ value, label: t(`settings.appearance.${value}` as MessageKey) }))}
                    className="max-w-[420px]"
                    data-testid="settings-appearance-choice"
                  />
                </SettingsCard>
                <SettingsCard title={t('settings.language')} description={t('settings.language.hint')} testId="settings-language">
                  <SegmentedControl
                    label={t('settings.language')}
                    value={data.language}
                    onChange={(preference) => { void setLanguage.run({ preference }); }}
                    items={[
                      { value: 'auto', label: t('settings.language.auto') },
                      { value: 'en-US', label: t('settings.language.en') },
                      { value: 'zh-CN', label: t('settings.language.zh') },
                    ]}
                    className="max-w-[420px]"
                    data-testid="settings-language-choice"
                  />
                </SettingsCard>
              </>
            ) : null}

            {data.section === 'downloads' ? (
              <SettingsCard title={t('settings.downloads.dir')} description={data.download?.downloadDir ?? ''} testId="settings-downloads">
                <view className="flex-row flex-wrap gap-2">
                  {data.download?.canChooseDirectory ? (
                    <Button label={t('settings.downloads.choose')} variant="secondary" clickAction={() => chooseDownloadDir.run()} />
                  ) : null}
                  {data.download && !data.download.usesDefaultDir ? (
                    <Button label={t('settings.downloads.reset')} variant="ghost" clickAction={() => resetDownloadDir.run()} />
                  ) : null}
                  <Button label={t('settings.downloads.open')} variant="ghost" endIcon="arrow-end" clickAction={() => openDownloads.run()} />
                </view>
              </SettingsCard>
            ) : null}

            {data.section === 'privacy' ? (
              <>
                {data.siteTabId ? (
                  <SettingsCard title={t('settings.privacy.site.title')} description={data.site?.host ?? ''} testId="settings-site">
                    <view className="gap-2">
                      <CheckboxInput label={t('privacy.cache')} bind={siteCache} />
                      <CheckboxInput label={t('privacy.siteData')} bind={siteData} />
                    </view>
                    <view className="flex-row">
                      <Button
                        label={t('settings.privacy.clearCurrentSite')}
                        variant="secondary"
                        isDisabled={!siteCache.value && !siteData.value}
                        disabledMessage={t('privacy.clear.nothingSelected')}
                        clickAction={() => run(clearSite.run({ cache: siteCache.value, siteData: siteData.value }))}
                      />
                    </view>
                  </SettingsCard>
                ) : null}
                <SettingsCard
                  title={t('privacy.clear.title')}
                  description={data.usage ? t('settings.privacy.usage', {
                    history: data.usage.historyEntries,
                    cacheSites: data.usage.cacheSites,
                    siteDataSites: data.usage.siteDataSites,
                    cookies: data.usage.cookies,
                  }) : undefined}
                  testId="settings-clear-data"
                >
                  <view className="flex-row flex-wrap gap-2">
                    <Button label={t('privacy.clear.open')} variant="secondary" onClick={() => clearing.set(true)} data-testid="settings-clear-data-open" />
                    <Button label={t('settings.privacy.viewHistory')} variant="ghost" endIcon="arrow-end" clickAction={() => openHistory.run()} />
                  </view>
                </SettingsCard>
              </>
            ) : null}

            {data.section === 'proxy' && data.proxy ? (
              <SettingsCard title={t('settings.proxy')} description={data.proxy.statusMessage || undefined} testId="settings-proxy">
                <SegmentedControl
                  label={t('settings.proxy.mode')}
                  value={data.proxy.mode}
                  onChange={(mode) => { void run(updateProxy.run({ mode: mode as (typeof PROXY_MODES)[number][0], host, port })); }}
                  items={PROXY_MODES.map(([value, label]) => ({ value, label: t(label) }))}
                  data-testid="settings-proxy-mode"
                />
                <view className="flex-row gap-3">
                  <TextInput label={t('settings.proxy.host')} value={host} onChange={(v) => proxyHost.set(v)} className="grow basis-0 min-w-0" />
                  <TextInput label={t('settings.proxy.port')} value={port} inputMode="numeric" onChange={(v) => proxyPort.set(v)} className="w-28" />
                </view>
                <view className="flex-row flex-wrap gap-2">
                  <Button
                    label={t('settings.proxy.apply')}
                    variant="primary"
                    clickAction={() => run(updateProxy.run({ mode: data.proxy?.mode ?? 'direct', host, port }))}
                  />
                  {data.proxy.mode === 'gfw_list' ? (
                    <Button label={t('settings.proxy.refreshRules')} variant="secondary" clickAction={() => run(refreshGfwList.run())} />
                  ) : null}
                </view>
              </SettingsCard>
            ) : null}

            {data.section === 'about' && data.appInfo ? (
              <SettingsCard title={data.appInfo.productName} testId="settings-about">
                <MetadataList
                  label={t('settings.about')}
                  items={[
                    { key: 'version', label: t('settings.about.version'), value: data.appInfo.version },
                    { key: 'sdk', label: t('settings.about.sdk'), value: data.appInfo.sdkVersion },
                    { key: 'webui', label: t('settings.about.webui'), value: data.appInfo.webuiVersion },
                    ...(data.appInfo.gitSha ? [{ key: 'commit', label: t('settings.about.commit'), value: data.appInfo.gitSha }] : []),
                  ]}
                />
              </SettingsCard>
            ) : null}
          </view>
        </scroll>
      </view>
      <ClearDataDialog
        isOpen={clearing.value}
        onOpenChange={(next) => clearing.set(next)}
        onClear={(input) => run(clearBrowsingData.run(input))}
      />
    </view>
  );
}
