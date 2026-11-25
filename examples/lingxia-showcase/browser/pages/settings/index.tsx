import { t, useAction, useLocal, useView, type Child, type MessageKey } from '@lingxia/view';
import { Badge, type BadgeStatusTone } from '@liulidesign/core/Badge';
import { Banner } from '@liulidesign/core/Banner';
import { Button } from '@liulidesign/core/Button';
import { Card } from '@liulidesign/core/Card';
import { CheckboxInput } from '@liulidesign/core/CheckboxInput';
import { IconButton } from '@liulidesign/core/IconButton';
import { Item, ItemList } from '@liulidesign/core/Item';
import { MetadataList } from '@liulidesign/core/MetadataList';
import { NavList } from '@liulidesign/core/NavList';
import { RadioGroup } from '@liulidesign/core/RadioGroup';
import { SegmentedControl } from '@liulidesign/core/SegmentedControl';
import { TextInput } from '@liulidesign/core/TextInput';
import { formatTimestamp } from '@liulidesign/core/Timestamp';
import { ToastRegion, useToast } from '@liulidesign/core/Toast';
import { ClearDataDialog } from '../../shared/ClearDataDialog';
import type { View } from './$types';

type Data = View['data'];
type Proxy = NonNullable<Data['proxy']>;
type Rule = Proxy['autoSwitchRules'][number];
type Notice = { message: string; tone?: 'success' | 'warning' } | null;

// The wire value is the Rust enum's; the catalogue key is camelCase.
const PROXY_MODES = [
  ['direct', 'directHint', 'settings.proxy.mode.direct'],
  ['gfw_list', 'gfwListHint', 'settings.proxy.mode.gfwList'],
  ['global', 'globalHint', 'settings.proxy.mode.global'],
] as const;

// The route reports its state as a code; the pill names it in the page's language.
const PROXY_STATE: Record<string, [string, BadgeStatusTone | null]> = {
  active: ['settings.proxy.state.active', 'success'],
  saved: ['settings.proxy.state.saved', null],
  pending: ['settings.proxy.state.pending', 'info'],
  disabled: ['settings.proxy.state.disabled', null],
  error: ['settings.proxy.state.error', 'danger'],
  unsupported: ['settings.proxy.state.unsupported', 'warning'],
};
const LIST_STATE: Record<string, [string, BadgeStatusTone | null]> = {
  ready: ['settings.proxy.list.ready', 'success'],
  empty: ['settings.proxy.list.empty', null],
  error: ['settings.proxy.list.error', 'danger'],
};

export function render(view: View) {
  return (
    <page data-testid="settings-page" className="bg-canvas text-text">
      <ToastRegion>
        <Settings {...view} />
      </ToastRegion>
    </page>
  );
}

function Pill({ state, table, testId }: { state: string; table: Record<string, [string, BadgeStatusTone | null]>; testId: string }) {
  const [key, tone] = table[state] ?? table.error;
  const label = t(key as MessageKey);
  return tone ? <Badge label={label} tone={tone} size="sm" data-testid={testId} /> : <Badge label={label} size="sm" data-testid={testId} />;
}

/** One card of settings: a heading, a line on what it does, an optional status, then its controls. */
function SettingsCard({ title, description, status, children, testId }: { title: string; description?: string; status?: Child; children: Child; testId?: string }) {
  return (
    <Card variant="outlined" padding="lg" data-testid={testId}>
      <view className="flex-row items-start gap-3">
        <view className="grow basis-0 min-w-0 gap-1">
          <text role="heading" headingLevel={2} className="text-callout font-semibold">{title}</text>
          {description ? <text className="text-footnote text-muted">{description}</text> : null}
        </view>
        {status ?? null}
      </view>
      {children}
    </Card>
  );
}

function Settings({ data, actions }: View) {
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
  const openHistory = useAction(actions.openHistory);
  const openDownloads = useAction(actions.openDownloads);
  const toast = useToast();

  // Actions answer with the toast's text, already translated.
  const run = async (outcome: Promise<{ ok: true; value: Notice | null } | { ok: false }>) => {
    const result = await outcome;
    const notice = result.ok ? result.value : null;
    if (notice) toast.show({ message: notice.message, tone: notice.tone ?? 'success', id: 'settings' });
  };

  const sections = (['appearance', 'downloads', 'privacy', 'proxy', 'about'] as const)
    .filter((id) => (id !== 'proxy' || data.proxy) && (id !== 'privacy' || data.hasPrivacy));

  return (
    <view className="grow min-h-0 flex-row">
      <view className="w-[240px] shrink-0 px-3 py-4 border-e border-line">
        <NavList
          label={t('settings.title')}
          sections={[{ id: 'sections', items: sections.map((id) => ({ value: id, label: t(`settings.${id}` as MessageKey) })) }]}
          value={data.section}
          onChange={(section) => { void setSection.run({ section: section as Data['section'] }); }}
          data-testid="settings-nav"
        />
      </view>

      <scroll className="grow basis-0 min-w-0">
        <view className="w-full max-w-[720px] px-8 py-6 gap-4">
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
                {setAppearance.error ? <text role="alert" className="text-footnote text-danger">{t('settings.saveFailed')}</text> : null}
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
                {setLanguage.error ? <text role="alert" className="text-footnote text-danger">{t('settings.saveFailed')}</text> : null}
              </SettingsCard>
            </>
          ) : null}

          {data.section === 'downloads' ? (
            <SettingsCard
              title={t('settings.downloads.dir')}
              description={data.download?.downloadDir ?? ''}
              status={data.download ? <Badge label={t(data.download.usesDefaultDir ? 'settings.downloads.default' : 'settings.downloads.custom')} size="sm" /> : undefined}
              testId="settings-downloads"
            >
              <view className="flex-row flex-wrap gap-2">
                {data.download?.canChooseDirectory ? (
                  <Button label={t('settings.downloads.choose')} variant="secondary" clickAction={() => chooseDownloadDir.run()} data-testid="settings-downloads-choose" />
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
                      clickAction={() => clearSite.run({ cache: siteCache.value, siteData: siteData.value })}
                    />
                  </view>
                </SettingsCard>
              ) : null}
              <ItemList title={t('settings.privacy.data')} data-testid="settings-usage">
                <Item
                  label={t('privacy.history')}
                  description={data.usage ? t('settings.privacy.historyCount', { count: data.usage.historyEntries }) : '…'}
                  endContent={<Button label={t('settings.privacy.viewHistory')} variant="ghost" size="sm" endIcon="arrow-end" clickAction={() => openHistory.run()} />}
                />
                <Item label={t('privacy.cache')} description={data.usage ? t('settings.privacy.cacheSites', { count: data.usage.cacheSites }) : '…'} />
                <Item
                  label={t('privacy.siteData')}
                  description={data.usage ? t('settings.privacy.cookies', { count: data.usage.cookies, sites: data.usage.siteDataSites }) : '…'}
                />
              </ItemList>
              <SettingsCard title={t('privacy.clear.title')} description={t('settings.privacy.clearHint')} testId="settings-clear-data">
                <view className="flex-row">
                  <Button label={t('privacy.clear.open')} variant="secondary" onClick={() => clearing.set(true)} data-testid="settings-clear-data-open" />
                </view>
              </SettingsCard>
            </>
          ) : null}

          {data.section === 'proxy' && data.proxy ? <ProxySection proxy={data.proxy} listFailure={data.listFailure} now={data.now} actions={actions} /> : null}

          {data.section === 'about' && data.appInfo ? (
            <SettingsCard title={data.appInfo.productName} testId="settings-about">
              <view className="flex-row items-center gap-4">
                <image src="public/LingXia.png" alt="" className="size-16 rounded-2xl object-contain" />
                <MetadataList
                  label={t('settings.about')}
                  items={[
                    { key: 'version', label: t('settings.about.version'), value: data.appInfo.version },
                    { key: 'sdk', label: t('settings.about.sdk'), value: data.appInfo.sdkVersion },
                    { key: 'webui', label: t('settings.about.webui'), value: data.appInfo.webuiVersion },
                    ...(data.appInfo.gitSha ? [{ key: 'commit', label: t('settings.about.commit'), value: data.appInfo.gitSha }] : []),
                  ]}
                  className="grow basis-0 min-w-0"
                />
              </view>
            </SettingsCard>
          ) : null}
        </view>
      </scroll>
      <ClearDataDialog
        isOpen={clearing.value}
        onOpenChange={(next) => clearing.set(next)}
        onClear={(input) => run(clearBrowsingData.run(input))}
      />
    </view>
  );
}

/**
 * Routing, the server, and (for Auto Switch) the rules and rule list. A field keeps what the user
 * typed until it is saved, then follows the saved settings again.
 */
function ProxySection({ proxy, listFailure, now, actions }: {
  proxy: Proxy;
  now: string;
  actions: View['actions'];
  listFailure: string | null;
}) {
  const isTwelveHour = useView().env.hourCycle === 'h12';
  const host = useLocal<string | null>('proxyHost', null);
  const port = useLocal<string | null>('proxyPort', null);
  const username = useLocal<string | null>('proxyUsername', null);
  const password = useLocal<string | null>('proxyPassword', null);
  const serverProblem = useLocal<{ field: string; message: string } | null>('serverProblem', null);
  const rules = useLocal<readonly Rule[] | null>('rules', null);
  const listUrl = useLocal<string | null>('listUrl', null);

  const setMode = useAction(actions.setProxyMode);
  const saveServer = useAction(actions.saveProxyServer);
  const saveAutoSwitch = useAction(actions.saveAutoSwitch);
  const refreshRules = useAction(actions.refreshRules);
  const isBusy = setMode.pending || saveServer.pending || refreshRules.pending;

  const shownRules = rules.value ?? proxy.autoSwitchRules;
  const shownUrl = listUrl.value ?? proxy.gfwlistSourceUrl;
  const editRules = async (next: readonly Rule[], url = shownUrl) => {
    rules.set(next);
    if (url !== proxy.gfwlistSourceUrl) listUrl.set(url);
    const result = await saveAutoSwitch.run({ rules: [...next], url });
    // A saved edit hands the fields back to the settings; a row still being written stays local.
    if (result.ok && result.value && next.every((rule) => rule.pattern.trim())) {
      rules.set(null);
      listUrl.set(null);
    }
  };

  const saveServerFields = async () => {
    serverProblem.set(null);
    const result = await saveServer.run({
      host: host.value ?? proxy.socks5Host,
      port: port.value ?? String(proxy.socks5Port),
      username: username.value ?? proxy.username,
      password: password.value ?? proxy.password,
    });
    if (!result.ok) return;
    if (result.value) { serverProblem.set(result.value); return; }
    host.set(null); port.set(null); username.set(null); password.set(null);
  };
  const problem = (field: string) => (serverProblem.value?.field === field ? { type: 'error' as const, message: serverProblem.value.message } : undefined);
  const isServerDirty = host.value !== null || port.value !== null || username.value !== null || password.value !== null;

  const stateDetail = proxy.status === 'active' && proxy.localProxyAddr
    ? t('settings.proxy.state.via', { address: proxy.localProxyAddr })
    : proxy.status === 'unsupported'
      ? t('settings.proxy.state.unsupportedDetail')
      : proxy.mode === 'direct' ? t('settings.proxy.state.directDetail') : undefined;

  return (
    <>
      <SettingsCard
        title={t('settings.proxy.mode')}
        description={stateDetail ?? t('settings.proxy.modeHint')}
        status={<Pill state={proxy.status} table={PROXY_STATE} testId="settings-proxy-state" />}
        testId="settings-proxy"
      >
        <RadioGroup
          label={t('settings.proxy.mode')}
          isLabelHidden
          variant="card"
          value={proxy.mode}
          isDisabled={isBusy}
          onChange={(mode) => { void setMode.run({ mode: mode as Proxy['mode'] }); }}
          items={PROXY_MODES.map(([value, hint, label]) => ({ value, label: t(label), description: t(`settings.proxy.mode.${hint}` as MessageKey) }))}
          data-testid="settings-proxy-mode"
        />
        {setMode.error ? <Banner title={t('settings.proxy.state.error')} message={t('settings.proxy.failedHint')} tone="danger" /> : null}
      </SettingsCard>

      {proxy.mode === 'gfw_list' ? (
        <>
          <SettingsCard title={t('settings.proxy.rules')} description={t('settings.proxy.rulesHint')} testId="settings-proxy-rules"
            status={saveAutoSwitch.pending ? <text className="text-footnote text-muted">{t('settings.proxy.rules.saving')}</text> : undefined}
          >
            <view className="gap-2">
              <RouteRow label={t('settings.proxy.rules.list')} hint={t('settings.proxy.rules.listHint')} viaProxy />
              {shownRules.map((rule, index) => (
                <view key={`rule-${index}`} className="flex-row items-center gap-2" data-testid={`settings-proxy-rule-${index}`}>
                  <TextInput
                    label={t('settings.proxy.rules.pattern')}
                    isLabelHidden
                    placeholder="example.com"
                    value={rule.pattern}
                    autoCapitalize="none"
                    hasSpellCheck={false}
                    onChange={(pattern) => { void editRules(shownRules.map((r, i) => (i === index ? { ...r, pattern } : r))); }}
                    className="grow basis-0 min-w-0"
                  />
                  <SegmentedControl
                    label={t('settings.proxy.rules.route', { pattern: rule.pattern || '…' })}
                    value={rule.action}
                    onChange={(action) => { void editRules(shownRules.map((r, i) => (i === index ? { ...r, action: action as Rule['action'] } : r))); }}
                    items={[
                      { value: 'proxy', label: t('settings.proxy.rules.viaProxy') },
                      { value: 'direct', label: t('settings.proxy.rules.viaDirect') },
                    ]}
                    size="sm"
                    className="w-44 shrink-0"
                  />
                  <IconButton
                    label={t('settings.proxy.rules.remove', { pattern: rule.pattern || '…' })}
                    icon="delete"
                    size="sm"
                    onClick={() => { void editRules(shownRules.filter((_, i) => i !== index)); }}
                  />
                </view>
              ))}
              <RouteRow label={t('settings.proxy.rules.default')} hint={t('settings.proxy.rules.defaultHint')} viaProxy={false} />
            </view>
            <view className="flex-row items-center gap-3">
              <Button
                label={t('settings.proxy.rules.add')}
                variant="secondary"
                size="sm"
                startIcon="add"
                onClick={() => rules.set([...shownRules, { pattern: '', action: 'proxy' }])}
                data-testid="settings-proxy-rule-add"
              />
              {saveAutoSwitch.error ? <text role="alert" className="text-footnote text-danger">{t('settings.proxy.rules.failed')}</text> : null}
            </view>
          </SettingsCard>

          <SettingsCard
            title={t('settings.proxy.list')}
            status={<Pill state={proxy.gfwlistStatus} table={LIST_STATE} testId="settings-proxy-list-state" />}
            testId="settings-proxy-list"
          >
            <TextInput
              label={t('settings.proxy.list.url')}
              description={t('settings.proxy.list.urlHint')}
              value={shownUrl}
              type="text"
              autoCapitalize="none"
              hasSpellCheck={false}
              onChange={(url) => { void editRules(shownRules, url); }}
              data-testid="settings-proxy-list-url"
            />
            <view className="flex-row items-center gap-3">
              <view className="grow basis-0 min-w-0 gap-0.5">
                <text className="text-footnote text-muted">{t('settings.proxy.list.updated')}</text>
                <text className="text-body tabular-nums" data-testid="settings-proxy-list-updated">
                  {refreshRules.pending
                    ? t('settings.proxy.list.downloading')
                    : proxy.gfwlistUpdatedAt
                      ? [formatTimestamp(proxy.gfwlistUpdatedAt, now, 'date-time', isTwelveHour), proxy.gfwlistRuleCount == null ? '' : t('settings.proxy.list.ruleCount', { count: proxy.gfwlistRuleCount })].filter(Boolean).join(' · ')
                      : t('settings.proxy.list.never')}
                </text>
              </view>
              <Button
                label={t('settings.proxy.list.download')}
                variant="secondary"
                isDisabled={isBusy && !refreshRules.pending}
                clickAction={() => refreshRules.run({ url: shownUrl })}
                data-testid="settings-proxy-list-download"
              />
            </view>
            {(listFailure || refreshRules.error) && !refreshRules.pending ? (
              <Banner
                title={listFailure ?? t('settings.proxy.list.failedHint')}
                // A saved list keeps routing, so the Ready badge beside this banner still holds.
                message={proxy.gfwlistUpdatedAt
                  ? t('settings.proxy.list.stillUsing', { time: formatTimestamp(proxy.gfwlistUpdatedAt, now, 'date-time', isTwelveHour) })
                  : undefined}
                tone="danger"
                action={{ label: t('settings.proxy.list.retry'), onClick: () => { void refreshRules.run({ url: shownUrl }); } }}
                data-testid="settings-proxy-list-failed"
              />
            ) : null}
          </SettingsCard>
        </>
      ) : null}

      <SettingsCard title={t('settings.proxy.server')} description={t('settings.proxy.serverHint')} testId="settings-proxy-server">
        <view className="flex-row gap-3">
          <TextInput
            label={t('settings.proxy.host')}
            value={host.value ?? proxy.socks5Host}
            placeholder="127.0.0.1"
            autoCapitalize="none"
            status={problem('host')}
            onChange={(v) => host.set(v)}
            className="grow basis-0 min-w-0"
            data-testid="settings-proxy-host"
          />
          <TextInput
            label={t('settings.proxy.port')}
            value={port.value ?? String(proxy.socks5Port)}
            inputMode="numeric"
            placeholder="1080"
            status={problem('port')}
            onChange={(v) => port.set(v)}
            className="w-32"
            data-testid="settings-proxy-port"
          />
        </view>
        <view className="flex-row gap-3">
          <TextInput
            label={t('settings.proxy.username')}
            optionalLabel={t('common.optional')}
            value={username.value ?? proxy.username}
            autoCapitalize="none"
            onChange={(v) => username.set(v)}
            className="grow basis-0 min-w-0"
          />
          <TextInput
            label={t('settings.proxy.password')}
            optionalLabel={t('common.optional')}
            type="password"
            value={password.value ?? proxy.password}
            onChange={(v) => password.set(v)}
            className="grow basis-0 min-w-0"
          />
        </view>
        <view className="flex-row items-center gap-3">
          <Button
            label={t('settings.proxy.saveServer')}
            variant={isServerDirty ? 'primary' : 'secondary'}
            isDisabled={isBusy && !saveServer.pending}
            clickAction={saveServerFields}
            data-testid="settings-proxy-save-server"
          />
          {saveServer.error ? <text role="alert" className="text-footnote text-danger">{t('settings.proxy.failedHint')}</text> : null}
        </view>
      </SettingsCard>
    </>
  );
}

/** A fixed row of the switch rules: what it matches and where it goes. */
function RouteRow({ label, hint, viaProxy }: { label: string; hint: string; viaProxy: boolean }) {
  return (
    <view className="flex-row items-center gap-3 px-3 py-2 rounded-control bg-text/4">
      <view className="grow basis-0 min-w-0">
        <text className="text-body">{label}</text>
        <text className="text-footnote text-muted truncate">{hint}</text>
      </view>
      {viaProxy
        ? <Badge label={t('settings.proxy.rules.viaProxy')} tone="accent" size="sm" />
        : <Badge label={t('settings.proxy.rules.viaDirect')} size="sm" />}
    </view>
  );
}
