import { useLocal, type View } from '@lingxia/view';
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
    label: 'Cloud',
    entries: [
      entry('api-other-app', 'Open another app', 'openOtherApp'),
      entry('api-cloud-auth', 'Cloud authentication', 'openCloud', { type: 'auth' }),
      entry('api-cloud-mqtt', 'Cloud MQTT', 'openCloud', { type: 'mqtt' }),
      entry('api-cloud-functions', 'Cloud functions', 'openCloud', { type: 'functions' }),
    ],
  },
  {
    id: 'bridge',
    label: 'Bridge',
    entries: [
      entry('api-stream', 'Stream', 'openStream', undefined, 'Async generator streaming with a chat demo'),
      entry('api-channel', 'Channel', 'openChannel', undefined, 'Bidirectional sessions with a live ticker'),
      entry('api-bridge', 'Bridge resilience', 'openBridge', undefined, 'Startup, streaming, reconnect and teardown'),
    ],
  },
  { id: 'navigation', label: 'Navigation', entries: [entry('api-deepseek', 'Open DeepSeek', 'openDeepSeek')] },
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
      entry('api-surface', 'Surface', 'openUi', { type: 'surface' }),
      entry('api-refresh', 'Pull to refresh', 'openRefresh'),
    ],
  },
  {
    id: 'system',
    label: 'System',
    entries: [
      entry('api-system-appBaseInfo', 'App information', 'openSystem', { type: 'appBaseInfo' }),
      entry('api-system-systemSetting', 'System settings', 'openSystem', { type: 'systemSetting' }),
      entry('api-system-autostart', 'Launch at login', 'openSystem', { type: 'autostart' }, 'Start automatically on macOS or Windows'),
      entry(
        'api-system-notification',
        'Local notifications',
        'openSystem',
        { type: 'notification' },
        'Post and cancel notifications from the Control app',
      ),
      entry('api-system-cache', 'Product cache', 'openSystem', { type: 'cache' }, 'View and clear app caches from the home app'),
      {
        ...entry('api-system-banner', 'Desktop banner', 'openSystem', { type: 'banner' }, 'A corner notification with optional actions'),
        desktopOnly: true,
      },
      entry('api-exit', 'Exit app', 'exitApp', undefined, 'Confirm before closing the app'),
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
      entry('api-wifi', 'WiFi', 'openWifi'),
      entry('api-share', 'Share', 'openShare', undefined, 'Share text, pages, images and files'),
      entry('api-clipboard', 'Clipboard', 'openClipboard', undefined, 'Copy and paste text and images'),
      entry('api-leaveguard', 'Leave confirmation', 'openLeaveGuard', undefined, 'Ask before discarding unsaved changes'),
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
      entry('api-file', 'Open file', 'openFile', { section: 'openFile' }),
      entry('api-choose-file', 'Choose file', 'openFile', { section: 'chooseFile' }),
    ],
  },
];

export function render({ data, actions, env }: View) {
  const expanded = useLocal<Record<string, boolean>>('api-sections', {});
  const desktop = env.os === 'macOS' || env.os === 'Windows';
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
              <row className="items-center justify-between gap-3">
                <text className="text-base">{group.label}</text>
                <Icon name={group.id} />
              </row>
            </button>
            {expanded.value[group.id] && (
              <column className="divide-y divide-line border-t border-line">
                {group.entries
                  .filter(item => !item.desktopOnly || desktop)
                  .map(item => (
                    <button
                      key={item.id}
                      data-testid={item.id}
                      onClick={() => actions[item.action](item.payload)}
                      className="px-4 py-3 min-h-11 rounded-none bg-transparent text-text text-left active:bg-surface-sunken"
                    >
                      <row className="items-center gap-3">
                        <column className="grow basis-0 min-w-0 gap-1">
                          <text className="text-sm">{item.label}</text>
                          {item.description && <text className="text-xs text-muted line-clamp-3">{item.description}</text>}
                        </column>
                        <Icon name={item.id === 'api-exit' ? 'exit' : 'chevron'} className="w-4 h-4 text-muted" />
                      </row>
                    </button>
                  ))}
              </column>
            )}
          </Card>
        ))}
      <Card className="">
        <button
          data-testid="api-location"
          onClick={() => actions.openLocation()}
          className="px-4 py-4 rounded-2xl bg-transparent text-text text-left active:bg-surface-sunken"
        >
          <row className="items-center justify-between">
            <text className="text-base">Location</text>
            <Icon name="location" />
          </row>
        </button>
      </Card>
    </Page>
  );
}
