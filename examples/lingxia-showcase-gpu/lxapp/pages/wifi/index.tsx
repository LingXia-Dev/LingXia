import { useLocal, type View } from '@lingxia/ui';
import '../../theme/index.css';
import { Button, EmptyState, Field, InfoList, InfoRow, Page, Section } from '../../shared/ui';

type Wifi = { ssid?: string; bssid?: string; secure?: boolean; signalStrength?: number; frequency?: number; connected?: boolean; state?: string };
type WifiEvent = Wifi & { id: string; time: string };
const percent = (value?: number) => (typeof value === 'number' ? `${value}%` : '--');

export function render({ data, actions }: View) {
  const ssid = useLocal('ssid', '');
  const password = useLocal('password', '');
  const enabled = !!data.wifiModuleEnabled;
  const listening = !!data.wifiListenerEnabled;
  const networks = (data.wifiList ?? []) as Wifi[];
  const connected = data.connectedWifi as Wifi | null;
  const events = (data.wifiConnectedEvents ?? []) as WifiEvent[];
  return (
    <Page>
      <Section title="WiFi Module" description="Initialize or stop WiFi module">
        <grid className="gap-3 grid-cols-2">
          <Button testId="wifi-start" disabled={enabled || !!data.scanning} onClick={() => actions.startWifi()}>
            Start WiFi
          </Button>
          <Button variant="destructive" testId="wifi-stop" disabled={!enabled || !!data.scanning} onClick={() => actions.stopWifi()}>
            Stop WiFi
          </Button>
        </grid>
        {data.statusText !== 'Ready' && (
          <text data-testid="wifi-status" className="text-xs text-muted">
            {String(data.statusText || '')}
          </text>
        )}
      </Section>
      {enabled && (
        <Section
          title="Connected WiFi"
          description="Get current WiFi connection info"
          action={
            <Button size="sm" testId="wifi-current" onClick={() => actions.getConnectedWifi()}>
              Get Info
            </Button>
          }
        >
          {!!connected && (
            <InfoList testId="wifi-connected">
              <InfoRow label="SSID" value={connected.ssid} />
              <InfoRow label="BSSID" value={connected.bssid} />
              <InfoRow label="Secure" value={connected.secure ? 'Yes' : 'No'} />
              <InfoRow label="Signal" value={percent(connected.signalStrength)} />
              <InfoRow label="Frequency" value={connected.frequency === undefined ? undefined : `${connected.frequency} MHz`} />
            </InfoList>
          )}
        </Section>
      )}
      {enabled && (
        <Section
          title="WiFi Connected Events"
          description="Listen to WiFi connection changes"
          action={
            <row className="gap-2">
              <Button size="sm" testId="wifi-listener-on" disabled={listening} onClick={() => actions.onWifiConnected()}>
                On
              </Button>
              <Button size="sm" variant="secondary" testId="wifi-listener-off" disabled={!listening} onClick={() => actions.offWifiConnected()}>
                Off
              </Button>
            </row>
          }
        >
          <row className="items-center justify-between">
            <text data-testid="wifi-listener-status" className="text-xs text-muted">{`Listening: ${listening ? 'On' : 'Off'}`}</text>
            {events.length > 0 && (
              <Button variant="ghost" size="sm" testId="wifi-clear-events" onClick={() => actions.clearWifiConnectedEvents()}>
                Clear
              </Button>
            )}
          </row>
          {!events.length ? (
            <text data-testid="wifi-events" className="text-sm text-muted">
              No events yet.
            </text>
          ) : (
            <column data-testid="wifi-events" className="gap-2">
              {events.map(event => (
                <column key={event.id} className="p-3 gap-1 rounded-lg border border-line">
                  <row className="justify-between">
                    <text className="text-xs text-muted">{event.time}</text>
                    <text className="text-xs text-muted">{percent(event.signalStrength)}</text>
                  </row>
                  <text className="text-sm font-semibold">{event.ssid || '--'}</text>
                  <text className="text-[11px] text-muted">
                    {`State: ${event.state ?? (event.connected === undefined ? '--' : event.connected ? 'Connected' : 'Disconnected')}`}
                  </text>
                  <text className="text-[11px] text-muted">{`Secure: ${event.secure === undefined ? '--' : event.secure ? 'Yes' : 'No'}`}</text>
                </column>
              ))}
            </column>
          )}
        </Section>
      )}
      {enabled && (
        <Section
          title="Scan WiFi Networks"
          description="Get list of available networks"
          action={
            <Button size="sm" testId="wifi-scan" disabled={!!data.scanning} onClick={() => actions.getWifiList()}>
              {data.scanning ? 'Scanning...' : 'Scan'}
            </Button>
          }
        >
          {data.wifiList !== null && (
            <text data-testid="wifi-scan-count" className="text-sm font-semibold">{`Available Networks (${networks.length})`}</text>
          )}
          {data.wifiList !== null && !networks.length && <EmptyState icon="device" title="No networks found" />}
          <scroll className="max-h-96 gap-2">
            {networks.map((wifi, index) => (
              <button
                key={wifi.bssid || `${wifi.ssid}-${index}`}
                data-testid={`wifi-network-${index}`}
                onClick={() => ssid.set(wifi.ssid || '')}
                className="p-3 rounded-lg border border-line bg-surface text-text text-left active:bg-surface-sunken"
              >
                <column className="gap-1">
                  <row className="items-center justify-between gap-2">
                    <text className="grow basis-0 min-w-0 text-sm font-semibold">{wifi.ssid || 'Hidden network'}</text>
                    <text className="px-2 py-1 rounded-full bg-accent-soft text-xs text-accent">{percent(wifi.signalStrength)}</text>
                  </row>
                  {!!wifi.bssid && <text className="text-xs text-muted">{`BSSID: ${wifi.bssid}`}</text>}
                  <text className="text-xs text-muted">{wifi.secure ? 'Security: Secured' : 'Security: Open'}</text>
                </column>
              </button>
            ))}
          </scroll>
        </Section>
      )}
      {enabled && (
        <Section
          title="Connect to WiFi"
          description={networks.length ? 'Select a network above or enter SSID manually' : 'Provide SSID and password'}
        >
          <Field testId="wifi-ssid" label="SSID" bind={ssid} placeholder={networks.length ? 'Enter SSID or select a network above' : 'SSID'} />
          <Field testId="wifi-password" label="Password" bind={password} type="password" placeholder="Password (optional)" />
          <Button
            testId="wifi-connect"
            disabled={!ssid.value.trim()}
            onClick={() => actions.connectWifi({ ssid: ssid.value.trim(), password: password.value || undefined })}
          >
            Connect
          </Button>
        </Section>
      )}
    </Page>
  );
}
