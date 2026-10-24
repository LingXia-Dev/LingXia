import { useLocal, type View, type Props } from '@lingxia/ui';
import '../../theme/index.css';

const primary = 'bg-linear-to-r from-blue-600 to-blue-500 text-white shadow-sm text-sm font-medium rounded-xl px-5 py-2.5';
const secondary = 'bg-linear-to-r from-surface-600 to-surface-500 text-white shadow-sm text-sm font-medium rounded-xl px-5 py-3';
const titles: Record<string, string> = { device: 'Device information', screen: 'Screen information', vibrate: 'Device vibration', dial: 'Phone call', networkType: 'Network type', localIP: 'Local IP addresses', networkStatus: 'Network status listener', orientation: 'Device orientation' };
function InfoRow({ label, value, testId, last = false }: { label: string; value: unknown; testId?: string; last?: boolean }) {
  const display = value === undefined || value === null || value === '' || (Array.isArray(value) && !value.length) ? '--' : Array.isArray(value) ? value.join('\n') : String(value);
  return <row className={`items-start justify-between gap-3 py-3 ${last ? '' : 'border-b border-line-200'}`}>
    <text className="text-sm text-gray-600 shrink-0">{label}</text>
    <column className="grow basis-0 items-end max-w-[72%]" style={{ minWidth: 0 }}><text testId={testId} className="text-right break-all text-sm font-semibold text-gray-800 bg-blue-50 rounded-lg px-3 py-1">{display}</text></column>
  </row>;
}
function Results({ children, testId, title, accent = 'bg-blue-500' }: Pick<Props, 'children' | 'testId'> & { title?: string; accent?: string }) {
  return <column className="p-5"><column testId={testId} className="bg-linear-to-br from-surface-50 to-surface rounded-xl border border-line-200 p-4">
    {title && <row className="items-center gap-2 pb-4"><spacer className={`w-1 h-4 rounded-full ${accent}`} /><text className="text-sm font-semibold text-gray-700">{title}</text></row>}
    {children}
  </column></column>;
}
function Header({ title, description, icon, children, divided = false, compact = false, tone = 'bg-linear-to-br from-blue-50 to-indigo-50' }: Pick<Props, 'children'> & { title: string; description: string; icon: string; divided?: boolean; compact?: boolean; tone?: string }) {
  return <row className={`items-center ${compact ? 'gap-3' : 'gap-4 p-5'} ${divided ? 'border-b border-line-100' : ''}`}>
    <column className={`shrink-0 rounded-xl items-center justify-center ${compact ? 'w-10 h-10' : 'w-12 h-12'} ${tone}`}><text className={compact ? 'text-xl' : 'text-2xl'}>{icon}</text></column>
    <column className="grow basis-0 gap-0.5" style={{ minWidth: 0 }}><text lines={3} className="text-sm font-semibold text-gray-800">{title}</text><text lines={4} className="text-xs text-gray-500">{description}</text></column>
    {children}
  </row>;
}
const yesNo = (value: unknown) => value === undefined ? '--' : value ? 'Yes' : 'No';
const number = (value: unknown, suffix = '') => typeof value === 'number' && Number.isFinite(value) ? `${Number.isInteger(value) ? value : value.toFixed(2)}${suffix}` : '--';

export function render({ data, actions, env }: View) {
  const phone = useLocal('phone', '');
  const requested = String(data.currentType ?? 'device');
  const type = titles[requested] ? requested : 'device';
  const info = (data.deviceInfo ?? {}) as Record<string, unknown>;
  const screen = (data.screenInfo ?? {}) as Record<string, unknown>;
  const network = (data.networkInfo ?? {}) as Record<string, unknown>;
  const changed = (data.networkChange ?? {}) as Record<string, unknown>;
  return <column testId="device-page" className="bg-linear-to-br from-surface-50 to-surface-100">
    <spacer style={{ height: env.insets.top }} />
    <row className="grow basis-0"><spacer style={{ width: env.insets.left }} />
      <scroll testId="page-scroll" className="grow basis-0 px-4 py-6 gap-6">
        <column className="items-center gap-2"><text testId="device-mode-title" className="text-2xl font-light text-gray-800">{titles[type]}</text><spacer className="w-16 h-0.5 bg-surface-400" /></column>
        <column className="bg-surface shadow-sm rounded-2xl border border-line-100">
          {type === 'device' && <column>
            <Header divided title="Get device information" description="Brand, model and OS version" icon={'\u{1f4f1}'}><button testId="device-get-info" onClick={() => actions.getDeviceInfo()} className={primary}>Get info</button></Header>
            {!!data.deviceInfo && <Results testId="device-info" title="Device information"><InfoRow label="Brand" value={info.brand} /><InfoRow label="Market name" value={info.marketName || info.model} /><InfoRow label="Model" testId="device-info-model" value={info.model} /><InfoRow label="OS name" testId="device-info-osName" value={info.osName} /><InfoRow last label="OS version" value={info.osVersion} /></Results>}
          </column>}
          {type === 'screen' && <column>
            <Header divided tone="bg-linear-to-br from-purple-50 to-pink-50" title="Get screen information" description="Screen dimensions and scale" icon={'\u{1f5a5}'}><button testId="device-screen-get-info" onClick={() => actions.getScreenInfo()} className={primary}>Get info</button></Header>
            {!!data.screenInfo && <Results testId="device-screen" title="Screen information" accent="bg-purple-500"><InfoRow label="Width" testId="device-screen-width" value={number(screen.width, 'px')} /><InfoRow label="Height" value={number(screen.height, 'px')} /><InfoRow last label="Scale" testId="device-screen-scale" value={number(screen.scale)} /></Results>}
          </column>}
          {type === 'networkType' && <column>
            <Header divided tone="bg-linear-to-br from-cyan-50 to-sky-50" title="Get network type" description="Wi-Fi, mobile data, Ethernet or offline" icon={'\u{1f310}'}><button testId="device-network-get-info" onClick={() => actions.getNetworkInfo()} className={primary}>Get info</button></Header>
            <Results testId="device-network"><InfoRow label="Connected" value={yesNo(network.isConnected)} /><InfoRow last label="Network type" testId="device-network-type" value={network.networkType} /></Results>
          </column>}
          {type === 'localIP' && <column>
            <Header divided tone="bg-linear-to-br from-teal-50 to-emerald-50" title="Get local IP addresses" description="IPv4 and IPv6 on the active network" icon={'\u{1f4e1}'}><button testId="device-network-get-info" onClick={() => actions.getNetworkInfo()} className="bg-linear-to-r from-teal-600 to-teal-500 text-white shadow-sm text-sm font-medium rounded-xl px-5 py-2.5">Get IP</button></Header>
            <Results testId="device-network"><InfoRow label="IPv4" testId="device-ipv4" value={network.ipv4} /><InfoRow last label="IPv6" testId="device-ipv6" value={network.ipv6} /></Results>
          </column>}
          {type === 'networkStatus' && <column className="p-6 gap-4">
            <grid columns={2} className="gap-3"><button testId="device-network-listen-start" onClick={() => actions.startNetworkChangeListen()} className="grow basis-0 bg-linear-to-r from-green-600 to-green-500 text-white shadow-sm text-sm font-medium rounded-xl py-3">Start listening</button><button testId="device-network-listen-stop" onClick={() => actions.stopNetworkChangeListen()} className={`grow basis-0 ${secondary}`}>Stop listening</button></grid>
            <column testId="device-network-status" className="bg-linear-to-br from-surface-50 to-surface rounded-xl border border-line-200 p-4"><InfoRow label="Listening" testId="device-network-listening" value={data.networkListening ? 'Yes' : 'No'} /><InfoRow label="Connected" value={yesNo(changed.isConnected)} /><InfoRow label="Network type" value={changed.networkType} /><InfoRow label="IPv4" value={changed.ipv4} /><InfoRow last label="IPv6" value={changed.ipv6} /></column>
          </column>}
          {type === 'vibrate' && <column className="p-6 gap-4">
            <Header compact tone="bg-linear-to-br from-green-50 to-emerald-50" title="Trigger vibration" description="Try a short or long vibration" icon={'\u{1f4f3}'} />
            <grid columns={2} className="gap-3"><button testId="device-vibrate-short" onClick={() => actions.vibrateShort()} className="bg-linear-to-r from-blue-600 to-blue-500 text-white shadow-sm text-sm font-medium rounded-xl py-3">Short</button><button testId="device-vibrate-long" onClick={() => actions.vibrateLong()} className="grow basis-0 bg-linear-to-r from-indigo-600 to-indigo-500 text-white shadow-sm text-sm font-medium rounded-xl py-3">Long</button></grid>
          </column>}
          {type === 'dial' && <column className="p-6 gap-5">
            <Header compact tone="bg-linear-to-br from-orange-50 to-red-50" title="Dial a phone number" description="Open the native phone dialer" icon={'\u{1f4de}'} />
            <column className="gap-3"><input testId="device-phone-number" bind={phone} keyboard="phone" placeholder="Enter phone number" className="bg-surface text-gray-800 text-sm px-4 py-3 rounded-xl border border-line-200" /><button testId="device-phone-call" enabled={!!phone.value.trim()} onClick={() => actions.makePhoneCall({ phoneNumber: phone.value.trim() })} className="bg-linear-to-r from-blue-600 to-blue-500 text-white shadow-sm text-sm font-medium rounded-xl py-3">Call</button></column>
          </column>}
          {type === 'orientation' && <column className="p-6 gap-4">
            <Header compact tone="bg-linear-to-br from-violet-50 to-indigo-50" title="Lock and follow orientation" description="Choose an orientation and listen for changes" icon={'\u{1f9ed}'} />
            <grid columns={2} className="gap-3"><button testId="device-orientation-portrait" onClick={() => actions.setOrientationPortrait()} className="bg-linear-to-r from-violet-600 to-violet-500 text-white shadow-sm text-sm font-medium rounded-xl py-3">Lock portrait</button><button testId="device-orientation-landscape" onClick={() => actions.setOrientationLandscape()} className="bg-linear-to-r from-indigo-600 to-indigo-500 text-white shadow-sm text-sm font-medium rounded-xl py-3">Lock landscape</button></grid>
            <grid columns={2} className="gap-3"><button testId="device-orientation-listen-start" onClick={() => actions.startDeviceOrientationListen()} className="grow basis-0 bg-linear-to-r from-green-600 to-green-500 text-white shadow-sm text-sm font-medium rounded-xl py-3">Start listening</button><button testId="device-orientation-listen-stop" onClick={() => actions.stopDeviceOrientationListen()} className={`grow basis-0 ${secondary}`}>Stop listening</button></grid>
            <column className="bg-linear-to-br from-surface-50 to-surface rounded-xl border border-line-200 p-4"><InfoRow label="Listening" value={data.orientationListening ? 'Yes' : 'No'} /><InfoRow label="Lock target" testId="device-orientation-lock" value={data.orientationLock} /><InfoRow last label="Current value" testId="device-orientation-value" value={data.deviceOrientationValue} /></column>
            <column className="bg-linear-to-br from-surface-50 to-surface rounded-xl border border-line-200 p-4 gap-3"><row className="items-center justify-between gap-3"><text className="text-sm font-semibold text-gray-700">Orientation events</text><button testId="device-orientation-clear" onClick={() => actions.clearOrientationEvents()} className="bg-linear-to-r from-surface-600 to-surface-500 text-white shadow-sm text-xs font-medium rounded-lg px-3 py-1.5">Clear logs</button></row>
              <scroll testId="device-orientation-event-scroll" className="bg-surface rounded-lg border border-line-200 p-3 max-h-56"><text testId="device-orientation-events" className="break-all text-xs text-gray-700">{(data.orientationEvents as string[] ?? []).join('\n') || '--'}</text></scroll>
            </column>
          </column>}
        </column>
      </scroll><spacer style={{ width: env.insets.right }} /></row>
    <spacer style={{ height: Math.max(env.insets.bottom, env.keyboard.height) }} />
  </column>;
}
