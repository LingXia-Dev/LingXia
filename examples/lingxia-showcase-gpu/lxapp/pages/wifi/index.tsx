import { useLocal, type Props, type View } from '@lingxia/ui';
import '../../theme/index.css';

type Wifi = { ssid?: string; bssid?: string; secure?: boolean; signalStrength?: number; frequency?: number; connected?: boolean; state?: string };
type WifiEvent = Wifi & { id: string; time: string };
const card = 'bg-surface rounded-2xl shadow-sm border border-line-100';
const disabled = 'bg-surface-200 text-gray-400';
const button = 'min-h-11 px-4 py-3 text-sm font-medium rounded-xl shadow-sm';
function Header({ icon, title, description, tint, separator = true, children }: Pick<Props, 'children'> & { icon: string; title: string; description: string; tint: string; separator?: boolean }) {
  return <row className={separator ? "items-center gap-3 px-5 py-5 border-b border-line-100" : "items-center gap-3 px-5 py-5"}>
    <column className={`w-12 h-12 shrink-0 items-center justify-center rounded-xl bg-linear-to-br ${tint}`}><text className="text-2xl">{icon}</text></column>
    <column className="grow basis-0 min-w-0 gap-0.5"><text className="text-sm font-semibold text-gray-800">{title}</text><text className="text-xs text-gray-500">{description}</text></column>
    {children}
  </row>;
}
function InfoRow({ label, value, suffix = '' }: { label: string; value?: string | number; suffix?: string }) {
  return <row className="items-center justify-between py-3 gap-3 border-b border-line-200">
    <text className="text-sm text-gray-600">{label}</text><text className="min-w-0 shrink text-sm font-semibold text-gray-800 px-3 py-1 bg-blue-50 rounded-lg break-all">{value === undefined || value === '' ? '--' : `${value}${suffix}`}</text>
  </row>;
}
export function render({ data, actions, env }: View) {
  const ssid = useLocal('ssid', '');
  const password = useLocal('password', '');
  const enabled = !!data.wifiModuleEnabled;
  const listening = !!data.wifiListenerEnabled;
  const networks = (data.wifiList ?? []) as Wifi[];
  const connected = data.connectedWifi as Wifi | null;
  const events = (data.wifiConnectedEvents ?? []) as WifiEvent[];
  const height = Math.max(1, env.viewport.height - env.insets.top - Math.max(env.insets.bottom, env.keyboard.height));
  return <column className="bg-linear-to-br from-surface-50 to-surface-100 text-gray-800">
    <spacer style={{ height: env.insets.top }} />
    <row style={{ height }}><spacer style={{ width: env.insets.left }} />
      <scroll testId="page-scroll" className="grow basis-0 min-w-0" style={{ height }}>
        <column className="px-4 py-6 gap-5">
          <column className="items-center gap-2 pb-1"><text className="text-2xl font-light text-center">WiFi Management</text><spacer className="w-16 h-0.5 bg-surface-400" /></column>
          <column className={card}>
            <Header icon="🧩" title="WiFi Module" description="Initialize or stop WiFi module" tint="from-emerald-50 to-green-50" />
            <column className="p-6 gap-3">
              <grid columns={2} className="gap-3">
                <button testId="wifi-start" enabled={!enabled && !data.scanning} onClick={() => actions.startWifi()} className={`${button} ${enabled ? disabled : 'bg-linear-to-r from-green-600 to-green-500 text-white'}`}>Start WiFi</button>
                <button testId="wifi-stop" enabled={enabled && !data.scanning} onClick={() => actions.stopWifi()} className={`${button} ${!enabled ? disabled : 'bg-linear-to-r from-red-600 to-red-500 text-white'}`}>Stop WiFi</button>
              </grid>
              {data.statusText !== 'Ready' && <text testId="wifi-status" className="text-xs text-gray-500">{String(data.statusText || '')}</text>}
            </column>
          </column>
          {enabled && <column className={card}>
            <Header separator={!!connected} icon="📱" title="Connected WiFi" description="Get current WiFi connection info" tint="from-blue-50 to-indigo-50"><button testId="wifi-current" onClick={() => actions.getConnectedWifi()} className={`${button} bg-linear-to-r from-blue-600 to-blue-500 text-white`}>Get Info</button></Header>
            {connected && <column testId="wifi-connected" className="p-5"><column className="rounded-xl border border-line-200 bg-linear-to-br from-surface-50 to-surface p-4">
              <row className="items-center gap-2 pb-4"><spacer className="w-1 h-4 bg-blue-500 rounded-full" /><text className="text-sm font-semibold text-gray-700">Connected Network</text></row>
              <InfoRow label="SSID" value={connected.ssid} /><InfoRow label="BSSID" value={connected.bssid} /><InfoRow label="Secure" value={connected.secure ? 'Yes' : 'No'} /><InfoRow label="Signal" value={connected.signalStrength} suffix="%" /><InfoRow label="Frequency" value={connected.frequency} suffix=" MHz" />
            </column></column>}
          </column>}
          {enabled && <column className={card}>
            <Header icon="🔔" title="WiFi Connected Events" description="Listen to WiFi connection changes" tint="from-amber-50 to-orange-50">
              <row className="gap-2"><button testId="wifi-listener-on" enabled={!listening} onClick={() => actions.onWifiConnected()} className={`min-h-11 min-w-11 px-2 py-2 rounded-lg text-xs font-medium ${listening ? disabled : 'bg-linear-to-r from-amber-500 to-orange-500 text-white'}`}>On</button><button testId="wifi-listener-off" enabled={listening} onClick={() => actions.offWifiConnected()} className={`min-h-11 min-w-11 px-2 py-2 rounded-lg text-xs font-medium ${!listening ? disabled : 'bg-linear-to-r from-surface-600 to-surface-500 text-white'}`}>Off</button></row>
            </Header>
            <column className="p-5 gap-3">
              <row className="items-center justify-between"><text testId="wifi-listener-status" className="text-xs text-gray-500">{`Listening: ${listening ? 'On' : 'Off'}`}</text>{events.length > 0 && <button testId="wifi-clear-events" onClick={() => actions.clearWifiConnectedEvents()} className="min-h-11 px-2 py-2 bg-transparent text-xs text-gray-500">Clear</button>}</row>
              {!events.length ? <text testId="wifi-events" className="text-sm text-gray-500">No events yet.</text> : <column testId="wifi-events" className="gap-3">{events.map(event => <column key={event.id} className="rounded-lg border border-line-200 bg-surface p-3 gap-1">
                <row className="justify-between pb-1"><text className="text-xs text-gray-500">{event.time}</text><text className="text-xs text-gray-500">{typeof event.signalStrength === 'number' ? `${event.signalStrength}%` : '--'}</text></row>
                <text className="text-sm font-semibold">{event.ssid || '--'}</text>
                {event.bssid && <text className="text-[11px] text-gray-500">{`BSSID: ${event.bssid}`}</text>}{typeof event.frequency === 'number' && <text className="text-[11px] text-gray-500">{`Frequency: ${event.frequency} MHz`}</text>}
                <text className="text-[11px] text-gray-500">{`State: ${event.state ?? (event.connected === undefined ? '--' : event.connected ? 'Connected' : 'Disconnected')}`}</text><text className="text-[11px] text-gray-500">{`Secure: ${event.secure === undefined ? '--' : event.secure ? 'Yes' : 'No'}`}</text>
              </column>)}</column>}
            </column>
          </column>}
          {enabled && <column className={card}>
            <Header icon="📋" title="Scan WiFi Networks" description="Get list of available networks" tint="from-purple-50 to-pink-50"><button testId="wifi-scan" enabled={!data.scanning} onClick={() => actions.getWifiList()} className={`${button} bg-linear-to-r from-purple-600 to-purple-500 text-white`}>{data.scanning ? 'Scanning...' : 'Scan'}</button></Header>
            {data.wifiList !== null && <column className="p-5"><column className="rounded-xl border border-line-200 bg-linear-to-br from-surface-50 to-surface p-4 gap-3">
              <row className="items-center gap-2"><spacer className="w-1 h-4 bg-purple-500 rounded-full" /><text testId="wifi-scan-count" className="text-sm font-semibold text-gray-700">{`Available Networks (${networks.length})`}</text></row>
              <scroll className="max-h-96 gap-3">{networks.map((wifi, index) => <button key={wifi.bssid || `${wifi.ssid}-${index}`} testId={`wifi-network-${index}`} accessibilityLabel={wifi.ssid || 'Hidden network'} onClick={() => ssid.set(wifi.ssid || '')} className="p-3 bg-surface rounded-lg border border-line-200 text-left">
                <column className="gap-1"><row className="items-center justify-between gap-2 pb-1"><text className="grow basis-0 min-w-0 text-sm font-semibold text-gray-800">{wifi.ssid || 'Hidden network'}</text><text className="text-xs px-2 py-1 rounded-full bg-blue-50 text-blue-600 dark:text-blue-400">{typeof wifi.signalStrength === 'number' ? `${wifi.signalStrength}%` : '--'}</text></row>
                  {wifi.bssid && <text className="text-xs text-gray-500">{`BSSID: ${wifi.bssid}`}</text>}{typeof wifi.frequency === 'number' && <text className="text-xs text-gray-500">{`Frequency: ${wifi.frequency} MHz`}</text>}<text className="text-xs text-gray-500">{wifi.secure ? 'Security: 🔒 Secured' : 'Security: 🔓 Open'}</text>
                </column>
              </button>)}</scroll>
            </column></column>}
          </column>}
          {enabled && <column className={card}>
            <Header icon="🔗" title="Connect to WiFi" description={networks.length ? 'Select a network above or enter SSID manually' : 'Provide SSID and password'} tint="from-emerald-50 to-lime-50" />
            <column className="p-6 gap-3"><input testId="wifi-ssid" bind={ssid} placeholder={networks.length ? 'Enter SSID or select a network above' : 'SSID'} className="w-full min-h-11 px-4 py-3 text-sm border border-line-200 rounded-xl bg-surface text-gray-800" /><input testId="wifi-password" bind={password} secure placeholder="Password (optional)" className="w-full min-h-11 px-4 py-3 text-sm border border-line-200 rounded-xl bg-surface text-gray-800" /><button testId="wifi-connect" enabled={!!ssid.value.trim()} onClick={() => actions.connectWifi({ ssid: ssid.value.trim(), password: password.value || undefined })} className={`${button} bg-linear-to-r from-emerald-600 to-emerald-500 text-white`}>Connect</button></column>
          </column>}
        </column>
      </scroll><spacer style={{ width: env.insets.right }} />
    </row><spacer style={{ height: Math.max(env.insets.bottom, env.keyboard.height) }} />
  </column>;
}
