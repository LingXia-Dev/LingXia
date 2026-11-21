import { t, useLocal, type View } from '@lingxia/view';
import { Card, Icon, Page, type IconName } from '../../shared/ui';
import '../../theme/index.css';

type Entry = { id: string; label: string; action: string; payload?: Record<string, string>; description?: string; desktopOnly?: boolean };
type Group = { id: IconName; label: string; entries: Entry[] };
const entry = (id: string, label: string, action: string, payload?: Record<string, string>, description?: string): Entry => ({
  id,
  label,
  action,
  payload,
  description,
});
const ui = (type: string, label: string) => entry(`api-ui-${type}`, label, 'openUi', { type });
const device = (id: string, type: string, label: string) => entry(`api-device-${id}`, label, 'openDevice', { type });
const media = (id: string, type: string, label: string) => entry(`api-media-${id}`, label, 'openMedia', { type });

export const groups: Group[] = [
  {
    id: 'cloud',
    label: 'api.group.cloud',
    entries: [
      entry('api-other-app', 'api.otherApp', 'openOtherApp'),
      entry('api-cloud-auth', 'api.cloudAuth', 'openCloud', { type: 'auth' }),
      entry('api-cloud-mqtt', 'api.cloudMqtt', 'openCloud', { type: 'mqtt' }),
      entry('api-cloud-functions', 'api.cloudFunctions', 'openCloud', { type: 'functions' }),
    ],
  },
  {
    id: 'bridge',
    label: 'api.group.bridge',
    entries: [
      entry('api-stream', 'api.stream', 'openStream', undefined, 'api.streamDetail'),
      entry('api-channel', 'api.channel', 'openChannel', undefined, 'api.channelDetail'),
      entry('api-bridge', 'api.bridge', 'openBridge', undefined, 'api.bridgeDetail'),
    ],
  },
  { id: 'navigation', label: 'api.group.navigation', entries: [entry('api-deepseek', 'api.deepseek', 'openDeepSeek')] },
  {
    id: 'interface',
    label: 'api.group.interface',
    entries: [
      ui('navigation', 'api.uiNavigation'),
      ui('toast', 'api.uiToast'),
      ui('modal', 'api.uiModal'),
      ui('navbar', 'api.uiNavbar'),
      ui('tabbar', 'api.uiTabbar'),
      ui('appearance', 'api.uiAppearance'),
      ui('actionsheet', 'api.uiActionsheet'),
      entry('api-surface', 'api.surface', 'openUi', { type: 'surface' }),
      entry('api-refresh', 'api.refresh', 'openRefresh'),
    ],
  },
  {
    id: 'system',
    label: 'api.group.system',
    entries: [
      entry('api-system-appBaseInfo', 'api.systemAppBaseInfo', 'openSystem', { type: 'appBaseInfo' }),
      entry('api-system-systemSetting', 'api.systemSystemSetting', 'openSystem', { type: 'systemSetting' }),
      entry('api-system-autostart', 'api.systemAutostart', 'openSystem', { type: 'autostart' }, 'api.systemAutostartDetail'),
      entry('api-system-notification', 'api.systemNotification',
        'openSystem',
        { type: 'notification' },
        'api.systemNotificationDetail',
      ),
      entry('api-system-cache', 'api.systemCache', 'openSystem', { type: 'cache' }, 'api.systemCacheDetail'),
      {
        ...entry('api-system-banner', 'api.systemBanner', 'openSystem', { type: 'banner' }, 'api.systemBannerDetail'),
        desktopOnly: true,
      },
      entry('api-exit', 'api.exit', 'exitApp', undefined, 'api.exitDetail'),
    ],
  },
  {
    id: 'device',
    label: 'api.group.device',
    entries: [
      device('device', 'device', 'api.deviceDevice'),
      device('screen', 'screen', 'api.deviceScreen'),
      device('vibrate', 'vibrate', 'api.deviceVibrate'),
      device('phone', 'dial', 'api.devicePhone'),
      device('orientation', 'orientation', 'api.deviceOrientation'),
      device('network', 'networkType', 'api.deviceNetwork'),
      device('localIP', 'localIP', 'api.deviceLocalIP'),
      device('networkStatus', 'networkStatus', 'api.deviceNetworkStatus'),
      entry('api-wifi', 'api.wifi', 'openWifi'),
      entry('api-share', 'api.share', 'openShare', undefined, 'api.shareDetail'),
      entry('api-clipboard', 'api.clipboard', 'openClipboard', undefined, 'api.clipboardDetail'),
      entry('api-leaveguard', 'api.leaveguard', 'openLeaveGuard', undefined, 'api.leaveguardDetail'),
    ],
  },
  {
    id: 'media',
    label: 'api.group.media',
    entries: [
      media('Pictures', 'Pictures', 'api.mediaPictures'),
      media('Videos', 'Videos', 'api.mediaVideos'),
      media('ScanCode', 'scanCode', 'api.mediaScanCode'),
      media('ImageInfo', 'imageInfo', 'api.mediaImageInfo'),
      media('VideoTools', 'videoTools', 'api.mediaVideoTools'),
      media('SaveToAlbum', 'saveToAlbum', 'api.mediaSaveToAlbum'),
    ],
  },
  {
    id: 'file',
    label: 'api.group.file',
    entries: [
      entry('api-file', 'api.file', 'openFile', { section: 'openFile' }),
      entry('api-choose-file', 'api.chooseFile', 'openFile', { section: 'chooseFile' }),
    ],
  },
];

export function render({ data, actions, env }: View) {
  const expanded = useLocal<Record<string, boolean>>('api-sections', {});
  const desktop = env.os === 'macos' || env.os === 'windows';
  const toggle = (id: string) => expanded.set({ ...expanded.value, [id]: !expanded.value[id] });
  return (
    <Page testId="api-page">
      <Card className="px-4 py-6 items-center gap-3">
        <Icon name="bolt" className="w-12 h-12 text-accent" />
        <text className="text-center text-base font-medium line-clamp-3">{t('api.intro')}</text>
      </Card>
      {groups
        .filter(group => group.id !== 'cloud' || Boolean(data.cloudAvailable))
        .map(group => (
          <Card key={group.id} className="">
            <button
              data-testid={`api-${group.id}-section`}
              aria-label={t('api.sectionState', { name: t(group.label), state: expanded.value[group.id] ? 'expanded' : 'collapsed' })}
              onClick={() => toggle(group.id)}
              className="px-4 py-4 rounded-2xl bg-transparent text-text text-left active:bg-surface-sunken"
            >
              <view className="flex-row items-center justify-between gap-3">
                <text className="text-base">{t(group.label)}</text>
                <Icon name={group.id} />
              </view>
            </button>
            {expanded.value[group.id] && (
              <view className="divide-y divide-line border-t border-line">
                {group.entries
                  .filter(item => !item.desktopOnly || desktop)
                  .map(item => (
                    <button
                      key={item.id}
                      data-testid={item.id}
                      onClick={() => actions[item.action](item.payload)}
                      className="px-4 py-3 min-h-11 rounded-none bg-transparent text-text text-left active:bg-surface-sunken"
                    >
                      <view className="flex-row items-center gap-3">
                        <view className="grow basis-0 min-w-0 gap-1">
                          <text className="text-sm">{t(item.label)}</text>
                          {item.description && <text className="text-xs text-muted line-clamp-3">{t(item.description)}</text>}
                        </view>
                        <Icon name={item.id === 'api-exit' ? 'exit' : 'chevron'} className="w-4 h-4 text-muted" />
                      </view>
                    </button>
                  ))}
              </view>
            )}
          </Card>
        ))}
      <Card className="">
        <button
          data-testid="api-location"
          onClick={() => actions.openLocation()}
          className="px-4 py-4 rounded-2xl bg-transparent text-text text-left active:bg-surface-sunken"
        >
          <view className="flex-row items-center justify-between">
            <text className="text-base">{t('api.location')}</text>
            <Icon name="location" />
          </view>
        </button>
      </Card>
    </Page>
  );
}
