import { type View } from '@lingxia/ui';
import '../../theme/index.css';
import { Button, ErrorState, InfoList, InfoRow, Page, Section, Segmented } from '../../shared/ui';

const titles: Record<string, string> = {
  appBaseInfo: 'app.getBaseInfo',
  systemSetting: 'getSystemSetting',
  autostart: 'app.autostart',
  notification: 'app.notification',
  banner: 'app.banner',
  cache: 'app.cache',
};
const yesNo = (value: unknown) => (value == null ? '--' : value ? 'Yes' : 'No');
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
const targets = [
  { value: 'activate', label: 'activate' },
  { value: 'native', label: 'route → native' },
  { value: 'page', label: 'page → this page' },
];
const backgrounds = ['system', 'light', 'dark', '#f4f5f7', '#1c1c1e'].map(value => ({ value, label: value }));

export function render({ data, actions, env }: View) {
  const type = String(data.currentType || 'appBaseInfo');
  const base = (data.appBaseInfo || {}) as Record<string, unknown>;
  const setting = (data.systemSetting || {}) as Record<string, unknown>;
  const desktop = env.os === 'macOS' || env.os === 'Windows';
  return (
    <Page testId="system-page">
      <text testId="system-mode-title" className="text-2xl font-light text-center">
        {titles[type]}
      </text>
      {type === 'appBaseInfo' && (
        <Section
          title="Fetch App Base Info"
          description="App identity (OS, product, versions) plus the language it renders in"
          action={
            <Button testId="system-get-base" onClick={() => actions.getBaseInfo()}>
              Get Info
            </Button>
          }
        >
          {!!data.appBaseInfo && (
            <InfoList>
              <InfoRow label="Display Language" value={data.displayLanguage} testId="system-base-language" />
              <InfoRow label="OS" value={base.os} testId="system-base-os" />
              <InfoRow label="Product Name" value={base.productName} testId="system-base-productName" />
              <InfoRow label="Product Version" value={base.version} testId="system-base-version" />
              <InfoRow label="SDK Version" value={base.sdkVersion} testId="system-base-sdkVersion" />
            </InfoList>
          )}
        </Section>
      )}
      {type === 'systemSetting' && (
        <Section
          title="Fetch System Setting"
          description="WiFi, location, and Bluetooth toggles"
          action={
            <Button testId="system-get-settings" onClick={() => actions.getSystemSetting()}>
              Get Info
            </Button>
          }
        >
          {!!data.systemSetting && (
            <InfoList>
              <InfoRow label="WiFi Enabled" value={yesNo(setting.wifiEnabled)} testId="system-settings-wifiEnabled" />
              <InfoRow label="Location Enabled" value={yesNo(setting.locationEnabled)} testId="system-settings-locationEnabled" />
              <InfoRow label="Bluetooth Enabled" value={yesNo(setting.bluetoothEnabled)} testId="system-settings-bluetoothEnabled" />
            </InfoList>
          )}
        </Section>
      )}
      {type === 'autostart' && (
        <Section
          title="Launch at Startup"
          description={data.autostartSupported ? 'Register this app as a login / startup item' : 'Not available on this platform'}
          action={
            data.autostartSupported ? (
              <switch
                testId="system-autostart-toggle"
                accessibilityLabel="Launch at startup"
                value={!!data.autostartEnabled}
                onChange={() => actions.toggleAutostart()}
              />
            ) : null
          }
        >
          <InfoList>
            <InfoRow label="Supported" value={yesNo(data.autostartSupported)} testId="system-autostart-status" />
            <InfoRow label="Enabled (OS)" value={yesNo(data.autostartEnabled)} />
          </InfoList>
          <ErrorState error={data.autostartError ? String(data.autostartError) : null} />
          <Button variant="secondary" size="sm" testId="system-autostart-refresh" onClick={() => actions.refreshAutostart()}>
            Re-read OS State
          </Button>
        </Section>
      )}
      {type === 'cache' && (
        <Section
          title="Product Cache"
          description="Reclaimable product cache - home lxapp only"
          action={
            <Button size="sm" testId="system-cache-clear" disabled={!!data.cacheBusy} onClick={() => actions.clearCache()}>
              {data.cacheBusy ? 'Clearing...' : 'Clear'}
            </Button>
          }
        >
          <InfoList>
            <InfoRow label="Reclaimable (estimate)" value={bytes(data.cacheBytes)} testId="system-cache-size" />
            <InfoRow label="Last Freed (estimate)" value={bytes(data.cacheFreedBytes)} />
            {!!data.cacheNotice && <InfoRow label="Result" value={data.cacheNotice} testId="system-cache-status" />}
          </InfoList>
          <ErrorState testId="system-cache-error" error={data.cacheError ? String(data.cacheError) : null} />
          <text className="text-xs text-muted">Running apps keep their private caches; estimates exclude WebView cache.</text>
          <Button variant="secondary" size="sm" testId="system-cache-refresh" disabled={!!data.cacheBusy} onClick={() => actions.refreshCacheSize()}>
            Re-read Size
          </Button>
        </Section>
      )}
      {type === 'notification' && (
        <Section
          title="Local Notifications"
          description={data.notificationSupported ? 'Post and cancel a local banner - Control app only' : 'Not available on this host'}
          action={
            data.notificationSupported ? (
              <Button size="sm" testId="system-notification-show" onClick={() => actions.showNotification()}>
                Show
              </Button>
            ) : null
          }
        >
          <text className="text-xs text-muted">
            page is navigateTo. route is a host-registered native location. Neither needs a product URL.
          </text>
          <Segmented
            testId="system-notification-target"
            value={String(data.notificationTarget || 'activate')}
            options={targets}
            onChange={value => actions.setNotificationTarget(value)}
          />
          <InfoList>
            <InfoRow label="Supported" value={yesNo(data.notificationSupported)} />
            <InfoRow label="Permission" value={data.notificationPermission} testId="system-notification-status" />
            <InfoRow label="Last id" value={data.notificationLastId} testId="system-notification-result" />
          </InfoList>
          <ErrorState error={data.notificationError ? String(data.notificationError) : null} />
          <grid className="gap-2 grid-cols-2">
            <Button variant="secondary" size="sm" testId="system-notification-refresh" onClick={() => actions.refreshNotification()}>
              Re-read Permission
            </Button>
            <Button variant="secondary" size="sm" testId="system-notification-cancel" onClick={() => actions.cancelNotification()}>
              Cancel
            </Button>
          </grid>
        </Section>
      )}
      {type === 'banner' && desktop && (
        <Section
          title="Desktop Banner"
          description={data.bannerSupported ? 'Toast and Prompt show a native card at the window top-right' : 'Not available on this host'}
        >
          <InfoList>
            <InfoRow label="Supported" value={yesNo(data.bannerSupported)} />
            <InfoRow label="Last result" value={data.bannerLast} testId="system-banner-last" />
          </InfoList>
          <ErrorState error={data.bannerError ? String(data.bannerError) : null} />
          <Button variant="secondary" size="sm" testId="system-banner-reread" onClick={() => actions.refreshBanner()}>
            Re-read support
          </Button>
          <text className="text-xs text-muted">Toast auto-dismisses. Prompt waits for Allow / Deny on the card.</text>
          <Segmented
            testId="system-banner-background"
            value={String(data.bannerBackground || 'system')}
            options={backgrounds}
            onChange={value => actions.setBannerBackground(value)}
          />
          {!!data.bannerSupported && (
            <grid className="gap-2 grid-cols-3">
              <Button size="sm" testId="system-banner-toast" disabled={!!data.bannerBusy} onClick={() => actions.showBannerToast()}>
                Toast
              </Button>
              <Button size="sm" testId="system-banner-prompt" disabled={!!data.bannerBusy} onClick={() => actions.showBannerPrompt()}>
                Prompt
              </Button>
              <Button
                variant="secondary"
                size="sm"
                testId="system-banner-dismiss"
                disabled={!data.bannerBusy}
                onClick={() => actions.dismissBanner()}
              >
                Dismiss
              </Button>
            </grid>
          )}
        </Section>
      )}
    </Page>
  );
}
