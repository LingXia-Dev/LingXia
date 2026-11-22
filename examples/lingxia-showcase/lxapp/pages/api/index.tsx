import { useLocal, type Child, type LinkTarget } from '@lingxia/view';
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
    label: 'Cloud',
    entries: [
      entry('api-other-app', 'Open another app', a => a.openOtherApp()),
      entry('api-cloud-auth', 'Cloud authentication', { page: 'cloud', params: { type: 'auth' } }),
      entry('api-cloud-mqtt', 'Cloud MQTT', { page: 'cloud', params: { type: 'mqtt' } }),
      entry('api-cloud-functions', 'Cloud functions', { page: 'cloud', params: { type: 'functions' } }),
    ],
  },
  {
    id: 'bridge',
    label: 'Bridge',
    entries: [
      entry('api-stream', 'Stream', { page: 'stream' }, 'Async generator streaming with a chat demo'),
      entry('api-channel', 'Channel', { page: 'channel' }, 'Bidirectional sessions with a live ticker'),
      entry('api-bridge', 'Bridge resilience', { page: 'bridge-repro' }, 'Startup, streaming, reconnect and teardown'),
    ],
  },
  { id: 'navigation', label: 'Navigation', entries: [entry('api-deepseek', 'Open DeepSeek', a => a.openDeepSeek())] },
  {
    id: 'interface',
    label: 'User Interface',
    entries: [
      ui('navigation', 'Page navigation'),
      ui('toast', 'Toast'),
      ui('modal', 'Modal'),
      ui('navbar', 'Navigation bar'),
      ui('tabbar', 'Tab bar'),
      ui('appearance', 'Appearance'),
      ui('actionsheet', 'Action sheet'),
      entry('api-surface', 'Surface', { page: 'ui', params: { type: 'surface' } }),
      entry('api-refresh', 'Pull to refresh', { page: 'pullToRefresh' }),
    ],
  },
  {
    id: 'system',
    label: 'System',
    entries: [
      entry('api-system-appBaseInfo', 'App information', { page: 'system', params: { type: 'appBaseInfo' } }),
      entry('api-system-systemSetting', 'System settings', { page: 'system', params: { type: 'systemSetting' } }),
      entry('api-system-autostart', 'Launch at login', { page: 'system', params: { type: 'autostart' } }, 'Start automatically on macOS or Windows'),
      entry(
        'api-system-notification',
        'Local notifications',
        { page: 'system', params: { type: 'notification' } },
        'Post and cancel notifications from the Control app',
      ),
      entry('api-system-cache', 'Product cache', { page: 'system', params: { type: 'cache' } }, 'View and clear app caches from the home app'),
      {
        ...entry('api-system-banner', 'Desktop banner', { page: 'system', params: { type: 'banner' } }, 'A corner notification with optional actions'),
        desktopOnly: true,
      },
      entry('api-exit', 'Exit app', a => a.exitApp(), 'Confirm before closing the app'),
    ],
  },
  {
    id: 'device',
    label: 'Device',
    entries: [
      device('device', 'device', 'Device Info'),
      device('screen', 'screen', 'Screen Info'),
      device('vibrate', 'vibrate', 'Vibration'),
      device('phone', 'dial', 'Phone Call'),
      device('orientation', 'orientation', 'Device Orientation'),
      device('network', 'networkType', 'Network Type'),
      device('localIP', 'localIP', 'Local IP Address'),
      device('networkStatus', 'networkStatus', 'Network Status Listener'),
      entry('api-wifi', 'WiFi', { page: 'wifi' }),
      entry('api-share', 'Share', { page: 'share' }, 'Share text, pages, images and files'),
      entry('api-clipboard', 'Clipboard', { page: 'clipboard' }, 'Copy and paste text and images'),
      entry('api-leaveguard', 'Leave confirmation', { page: 'leaveGuard' }, 'Ask before discarding unsaved changes'),
    ],
  },
  {
    id: 'media',
    label: 'Media & Camera',
    entries: [
      media('Pictures', 'Pictures', 'Pictures'),
      media('Videos', 'Videos', 'Videos'),
      media('ScanCode', 'scanCode', 'Scan code'),
      media('ImageInfo', 'imageInfo', 'Image tools'),
      media('VideoTools', 'videoTools', 'Video tools'),
      media('SaveToAlbum', 'saveToAlbum', 'Save to album'),
    ],
  },
  {
    id: 'file',
    label: 'File',
    entries: [
      entry('api-file', 'Open file', { page: 'file', params: { section: 'openFile' } }),
      entry('api-choose-file', 'Choose file', { page: 'file', params: { section: 'chooseFile' } }),
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
        <text className="text-center text-base font-medium line-clamp-3">Explore what you can do with LingXia.</text>
      </Card>
      {groups
        .filter(group => group.id !== 'cloud' || Boolean(data.cloudAvailable))
        .map(group => (
          <Card key={group.id} className="">
            <button
              data-testid={`api-${group.id}-section`}
              aria-label={`${group.label}, ${expanded.value[group.id] ? 'expanded' : 'collapsed'}`}
              onClick={() => toggle(group.id)}
              className="px-4 py-4 rounded-2xl bg-transparent text-text text-left active:bg-surface-sunken"
            >
              <view className="flex-row items-center justify-between gap-3">
                <text className="text-base">{group.label}</text>
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
                          <text className="text-sm">{item.label}</text>
                          {item.description && <text className="text-xs text-muted line-clamp-3">{item.description}</text>}
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
            <text className="text-base">Location</text>
            <Icon name="location" />
          </view>
        </link>
      </Card>
    </Page>
  );
}
