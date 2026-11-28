import { t, useLocal, type Child, type LinkTarget, type MessageKey } from '@lingxia/view';
import { Card, Icon, Page, type IconName } from '../../shared/ui';
import '../../theme/index.css';
import type { View } from './$types';

// An entry either opens a page (`LinkTarget`) or runs an action that decides for itself.
type Go = LinkTarget | ((actions: View['actions']) => Promise<unknown>);
type Entry = { id: string; label: string; go: Go; description?: string; desktopOnly?: boolean };
type Group = { id: IconName; label: string; entries: Entry[] };
const entry = (id: string, label: string, go: Go, description?: string): Entry => ({ id, label, go, description });
const ui = (type: string, label: string) => entry(`api-ui-${type}`, label, { page: 'ui', params: { type } });
const device = (id: string, type: string, label: string) => entry(`api-device-${id}`, label, { page: 'device', params: { type } });
const media = (id: string, type: string, label: string) => entry(`api-media-${id}`, label, { page: 'media', params: { type } });

export const groups: Group[] = [
  {
    id: 'cloud',
    label: 'api.group.cloud',
    entries: [
      entry('api-other-app', 'api.otherApp', a => a.openOtherApp()),
      entry('api-cloud-auth', 'api.cloudAuth', { page: 'cloud', params: { type: 'auth' } }),
      entry('api-cloud-mqtt', 'api.cloudMqtt', { page: 'cloud', params: { type: 'mqtt' } }),
      entry('api-cloud-functions', 'api.cloudFunctions', { page: 'cloud', params: { type: 'functions' } }),
    ],
  },
  {
    id: 'bridge',
    label: 'api.group.bridge',
    entries: [
      entry('api-stream', 'api.stream', { page: 'stream' }, 'api.streamDetail'),
      entry('api-channel', 'api.channel', { page: 'channel' }, 'api.channelDetail'),
      entry('api-native', 'api.native', { page: 'native' }, 'api.nativeDetail'),
    ],
  },
  {
    id: 'navigation',
    label: 'api.group.navigation',
    entries: [
      entry('api-deepseek', 'api.deepseek', a => a.openDeepSeek()),
      entry('api-liuli-gallery', 'api.liuliGallery', a => a.openBundledApp('liuli-gallery')),
      entry('api-showcase-web', 'api.showcaseWeb', a => a.openBundledApp('lingxia-showcase-web')),
    ],
  },
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
      entry('api-surface', 'api.surface', { page: 'ui', params: { type: 'surface' } }),
      entry('api-refresh', 'api.refresh', { page: 'pullToRefresh' }),
    ],
  },
  {
    id: 'system',
    label: 'api.group.system',
    entries: [
      entry('api-system-appBaseInfo', 'api.systemAppBaseInfo', { page: 'system', params: { type: 'appBaseInfo' } }),
      entry('api-system-systemSetting', 'api.systemSystemSetting', { page: 'system', params: { type: 'systemSetting' } }),
      entry('api-system-autostart', 'api.systemAutostart', { page: 'system', params: { type: 'autostart' } }, 'api.systemAutostartDetail'),
      entry(
        'api-system-notification',
        'api.systemNotification',
        { page: 'system', params: { type: 'notification' } },
        'api.systemNotificationDetail',
      ),
      entry('api-system-cache', 'api.systemCache', { page: 'system', params: { type: 'cache' } }, 'api.systemCacheDetail'),
      {
        ...entry('api-system-banner', 'api.systemBanner', { page: 'system', params: { type: 'banner' } }, 'api.systemBannerDetail'),
        desktopOnly: true,
      },
      entry('api-exit', 'api.exit', a => a.exitApp(), 'api.exitDetail'),
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
      entry('api-wifi', 'api.wifi', { page: 'wifi' }),
      entry('api-share', 'api.share', { page: 'share' }, 'api.shareDetail'),
      entry('api-clipboard', 'api.clipboard', { page: 'clipboard' }, 'api.clipboardDetail'),
      entry('api-leaveguard', 'api.leaveguard', { page: 'leaveGuard' }, 'api.leaveguardDetail'),
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
      entry('api-file', 'api.file', { page: 'file', params: { section: 'openFile' } }),
      entry('api-choose-file', 'api.chooseFile', { page: 'file', params: { section: 'chooseFile' } }),
    ],
  },
];

const rowClass = 'px-4 py-3 min-h-11 rounded-none bg-transparent text-text text-left active:bg-surface-sunken';

function EntryRow({ item, actions, children }: { item: Entry; actions: View['actions']; children: Child }) {
  const go = item.go;
  return typeof go === 'function' ? (
    <button data-testid={item.id} onClick={() => void go(actions)} className={rowClass}>{children}</button>
  ) : (
    <link data-testid={item.id} to={go} className={rowClass}>{children}</link>
  );
}

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
              aria-label={t('api.sectionState', { name: t(group.label as MessageKey), state: expanded.value[group.id] ? 'expanded' : 'collapsed' })}
              onClick={() => toggle(group.id)}
              className="px-4 py-4 rounded-2xl bg-transparent text-text text-left active:bg-surface-sunken"
            >
              <view className="flex-row items-center justify-between gap-3">
                <text className="text-base">{t(group.label as MessageKey)}</text>
                <Icon name={group.id} />
              </view>
            </button>
            {expanded.value[group.id] && (
              <view className="divide-y divide-line border-t border-line">
                {group.entries
                  .filter(item => !item.desktopOnly || desktop)
                  .map(item => (
                    <EntryRow key={item.id} item={item} actions={actions}>
                      <view className="flex-row items-center gap-3">
                        <view className="grow basis-0 min-w-0 gap-1">
                          <text className="text-sm">{t(item.label as MessageKey)}</text>
                          {item.description && <text className="text-xs text-muted line-clamp-3">{t(item.description as MessageKey)}</text>}
                        </view>
                        <Icon name={item.id === 'api-exit' ? 'exit' : 'chevron'} className="w-4 h-4 text-muted" />
                      </view>
                    </EntryRow>
                  ))}
              </view>
            )}
          </Card>
        ))}
      <Card className="">
        <link
          data-testid="api-location"
          to={{ page: 'location' }}
          className="px-4 py-4 rounded-2xl bg-transparent text-text text-left active:bg-surface-sunken"
        >
          <view className="flex-row items-center justify-between">
            <text className="text-base">{t('api.location')}</text>
            <Icon name="location" />
          </view>
        </link>
      </Card>
    </Page>
  );
}
