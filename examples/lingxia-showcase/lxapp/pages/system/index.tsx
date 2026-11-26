import { t } from '@lingxia/view';
import '../../theme/index.css';
import { Button, ErrorState, InfoList, InfoRow, Page, Section, Segmented } from '../../shared/ui';
import type { View } from './$types';

const titles: Record<string, string> = {
  appBaseInfo: 'app.getBaseInfo',
  systemSetting: 'getSystemSetting',
  autostart: 'host.autostart',
  notification: 'host.notification',
  banner: 'host.banner',
  cache: 'app.cache',
};
const yesNo = (value: unknown) => (value == null ? '--' : value ? t('common.yes') : t('common.no'));
function bytes(value: unknown): string {
  if (typeof value !== 'number') return '--';
  if (value < 1024) return `${value} B`;
  let size = value / 1024,
    unit = 0;
  while (size >= 1024 && unit < 2) {
    size /= 1024;
    unit++;
  }
  return `${size.toFixed(1)} ${['KB', 'MB', 'GB'][unit]}`;
}
const targets = () => [
  { value: 'activate', label: 'activate' },
  { value: 'native', label: 'route → native' },
  { value: 'page', label: 'page → this page' },
].map(target => ({ ...target, label: target.value === 'page' ? `page → ${t('system.thisPage')}` : target.label }));
const backgrounds = ['system', 'light', 'dark', '#f4f5f7', '#1c1c1e'].map(value => ({ value, label: value }));

export function render({ data, actions, env }: View) {
  const type = String(data.currentType || 'appBaseInfo');
  const base = (data.appBaseInfo || {}) as Record<string, unknown>;
  const setting = (data.systemSetting || {}) as Record<string, unknown>;
  const desktop = env.os === 'macos' || env.os === 'windows';
  return (
    <Page testId="system-page" data={{ 'data-mode': type }}>
      <text data-testid="system-mode-title" className="text-2xl font-light text-center">
        {titles[type]}
      </text>
      {type === 'appBaseInfo' && (
        <Section
          testId="system-base-result"
          title={t('system.fetchAppBaseInfo')}
          description={t('system.appIdentityOsProduct')}
          action={
            <Button testId="system-base-info" onClick={() => actions.getBaseInfo()}>
              {t('system.getInfo')}
            </Button>
          }
        >
          {!!data.appBaseInfo && (
            <InfoList>
              <InfoRow label={t('system.displayLanguage')} value={data.displayLanguage} testId="system-base-language" />
              <InfoRow label="OS" value={base.os} testId="system-base-os" />
              <InfoRow label={t('system.productName')} value={base.productName} testId="system-base-productName" />
              <InfoRow label={t('system.productVersion')} value={base.version} testId="system-base-version" />
              <InfoRow label={t('system.sdkVersion')} value={base.sdkVersion} testId="system-base-sdkVersion" />
            </InfoList>
          )}
        </Section>
      )}
      {type === 'systemSetting' && (
        <Section
          testId="system-setting-result"
          title={t('system.fetchSystemSetting')}
          description={t('system.wifiLocationAndBluetooth')}
          action={
            <Button testId="system-setting-info" onClick={() => actions.getSystemSetting()}>
              {t('system.getInfo')}
            </Button>
          }
        >
          {!!data.systemSetting && (
            <InfoList>
              <InfoRow label={t('system.wifiEnabled')} value={yesNo(setting.wifiEnabled)} testId="system-settings-wifiEnabled" />
              <InfoRow label={t('system.locationEnabled')} value={yesNo(setting.locationEnabled)} testId="system-settings-locationEnabled" />
            </InfoList>
          )}
        </Section>
      )}
      {type === 'autostart' && (
        <Section
          title={t('system.launchAtStartup')}
          description={data.autostartSupported ? t('system.autostartDetail') : t('system.notOnPlatform')}
          action={
            data.autostartSupported ? (
              <switch
                data-testid="system-autostart-toggle"
                aria-label={t('system.launchAtStartup2')}
                value={!!data.autostartEnabled}
                onChange={() => actions.toggleAutostart()}
              />
            ) : null
          }
        >
          <InfoList>
            <InfoRow label={t('system.supported')} value={yesNo(data.autostartSupported)} testId="system-autostart-status" />
            <InfoRow label={t('system.enabledOs')} value={yesNo(data.autostartEnabled)} />
          </InfoList>
          <ErrorState error={data.autostartError || null} />
          <Button variant="secondary" size="sm" testId="system-autostart-refresh" onClick={() => actions.refreshAutostart()}>
            {t('system.reReadOsState')}
          </Button>
        </Section>
      )}
      {type === 'cache' && (
        <Section
          testId="system-cache-panel"
          title={t('system.productCache')}
          description={t('system.reclaimableProductCacheHome')}
          action={
            <Button size="sm" testId="system-cache-clear" disabled={!!data.cacheBusy} onClick={() => actions.clearCache()}>
              {data.cacheBusy ? t('system.clearing') : t('system.clear')}
            </Button>
          }
        >
          <InfoList>
            <InfoRow label={t('system.reclaimableEstimate')} value={bytes(data.cacheBytes)} testId="system-cache-size" />
            <InfoRow label={t('system.lastFreedEstimate')} value={bytes(data.cacheFreedBytes)} />
            {!!data.cacheNotice && <InfoRow label={t('system.result')} value={data.cacheNotice} testId="system-cache-status" />}
          </InfoList>
          <ErrorState testId="system-cache-error" error={data.cacheError || null} />
          <text className="text-xs text-muted">{t('system.runningAppsKeepTheir')}</text>
          <Button variant="secondary" size="sm" testId="system-cache-refresh" disabled={!!data.cacheBusy} onClick={() => actions.refreshCacheSize()}>
            {t('system.reReadSize')}
          </Button>
        </Section>
      )}
      {type === 'notification' && (
        <Section
          title={t('system.localNotifications')}
          description={data.notificationSupported ? t('system.notificationDetail') : t('system.notOnHost')}
          action={
            data.notificationSupported ? (
              <Button size="sm" testId="system-notification-show" onClick={() => actions.showNotification()}>
                {t('system.show')}
              </Button>
            ) : null
          }
        >
          <text className="text-xs text-muted">
            {t('system.pageIsNavigatetoRoute')}
          </text>
          <Segmented
            testId="system-notification-target"
            value={String(data.notificationTarget || 'activate')}
            options={targets()}
            onChange={value => actions.setNotificationTarget(value)}
          />
          <InfoList>
            <InfoRow label={t('system.supported')} value={yesNo(data.notificationSupported)} />
            <InfoRow label={t('system.permission')} value={data.notificationPermission} testId="system-notification-status" />
            <InfoRow label={t('system.lastId')} value={data.notificationLastId} testId="system-notification-result" />
          </InfoList>
          <ErrorState error={data.notificationError || null} />
          <view className="grid gap-2 grid-cols-2">
            <Button variant="secondary" size="sm" testId="system-notification-refresh" onClick={() => actions.refreshNotification()}>
              {t('system.reReadPermission')}
            </Button>
            <Button variant="secondary" size="sm" testId="system-notification-cancel" onClick={() => actions.cancelNotification()}>
              {t('system.cancel')}
            </Button>
          </view>
        </Section>
      )}
      {type === 'banner' && desktop && (
        <Section
          testId="system-banner-panel"
          title={t('system.desktopBanner')}
          description={data.bannerSupported ? t('system.bannerDetail') : t('system.notOnHost')}
        >
          <InfoList>
            <InfoRow label={t('system.supported')} value={yesNo(data.bannerSupported)} />
            <InfoRow label={t('system.lastResult')} value={data.bannerLast} testId="system-banner-last" />
          </InfoList>
          <ErrorState error={data.bannerError || null} />
          <Button variant="secondary" size="sm" testId="system-banner-reread" onClick={() => actions.refreshBanner()}>
            {t('system.reReadSupport')}
          </Button>
          <text className="text-xs text-muted">{t('system.toastAutoDismissesPrompt')}</text>
          <Segmented
            testId="system-banner-background"
            value={String(data.bannerBackground || 'system')}
            options={backgrounds}
            onChange={value => actions.setBannerBackground(value)}
          />
          {!!data.bannerSupported && (
            <view className="grid gap-2 grid-cols-3">
              <Button size="sm" testId="system-banner-toast" disabled={!!data.bannerBusy} onClick={() => actions.showBannerToast()}>
                {t('system.toast')}
              </Button>
              <Button size="sm" testId="system-banner-prompt" disabled={!!data.bannerBusy} onClick={() => actions.showBannerPrompt()}>
                {t('system.prompt')}
              </Button>
              <Button
                variant="secondary"
                size="sm"
                testId="system-banner-dismiss"
                disabled={!data.bannerBusy}
                onClick={() => actions.dismissBanner()}
              >
                {t('system.dismiss')}
              </Button>
            </view>
          )}
        </Section>
      )}
    </Page>
  );
}
