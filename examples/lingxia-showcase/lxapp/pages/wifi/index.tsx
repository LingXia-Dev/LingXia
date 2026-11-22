import { t, useLocal } from '@lingxia/view';
import '../../theme/index.css';
import { said } from '../../shared/lib/said';
import { Button, EmptyState, Field, InfoList, InfoRow, Page, Section } from '../../shared/ui';
import type { View } from './$types';

type Wifi = { ssid?: string; bssid?: string; secure?: boolean; signalStrength?: number; frequency?: number; connected?: boolean; state?: string };
type WifiEvent = Wifi & { id: string; time: string };
const percent = (value?: number) => (typeof value === 'number' ? `${value}%` : '--');

export function render({ data, actions }: View) {
  const ssid = useLocal('ssid', '');
  const password = useLocal('password', '');
  const listening = !!data.wifiListenerEnabled;
  const networks = (data.wifiList ?? []) as Wifi[];
  const connected = data.connectedWifi as Wifi | null;
  const events = (data.wifiConnectedEvents ?? []) as WifiEvent[];
  return (
    <Page>
      {data.statusText !== 'wifi.state.ready' && (
        <text data-testid="wifi-status" className="text-xs text-muted">
          {said(String(data.statusText || ''), data.statusArgs as Record<string, unknown>)}
        </text>
      )}
      <Section
        title={t('wifi.connectedWifi')}
        description={t('wifi.getCurrentWifiConnection')}
        action={
          <Button size="sm" testId="wifi-current" onClick={() => actions.getConnectedWifi()}>
            {t('wifi.getInfo')}
          </Button>
        }
      >
        {!!connected && (
          <InfoList testId="wifi-connected">
            <InfoRow label="SSID" value={connected.ssid} />
            <InfoRow label="BSSID" value={connected.bssid} />
            <InfoRow label={t('wifi.secure')} value={connected.secure ? 'Yes' : 'No'} />
            <InfoRow label={t('wifi.signal')} value={percent(connected.signalStrength)} />
            <InfoRow label={t('wifi.frequency')} value={connected.frequency === undefined ? undefined : `${connected.frequency} MHz`} />
          </InfoList>
        )}
      </Section>
      <Section
        title={t('wifi.wifiConnectedEvents')}
        description={t('wifi.listenToWifiConnection')}
        action={
          <view className="flex-row gap-2">
            <Button size="sm" testId="wifi-listener-on" disabled={listening} onClick={() => actions.onWifiConnected()}>
              {t('wifi.on')}
            </Button>
            <Button size="sm" variant="secondary" testId="wifi-listener-off" disabled={!listening} onClick={() => actions.offWifiConnected()}>
              {t('wifi.off')}
            </Button>
          </view>
        }
      >
        <view className="flex-row items-center justify-between">
          <text data-testid="wifi-listener-status" className="text-xs text-muted">{`Listening: ${listening ? 'On' : 'Off'}`}</text>
          {events.length > 0 && (
            <Button variant="ghost" size="sm" testId="wifi-clear-events" onClick={() => actions.clearWifiConnectedEvents()}>
              {t('wifi.clear')}
            </Button>
          )}
        </view>
        {!events.length ? (
          <text data-testid="wifi-events" className="text-sm text-muted">
            {t('wifi.noEventsYet')}
          </text>
        ) : (
          <view data-testid="wifi-events" className="gap-2">
            {events.map(event => (
              <view key={event.id} className="p-3 gap-1 rounded-lg border border-line">
                <view className="flex-row justify-between">
                  <text className="text-xs text-muted">{event.time}</text>
                  <text className="text-xs text-muted">{percent(event.signalStrength)}</text>
                </view>
                <text className="text-sm font-semibold">{event.ssid || '--'}</text>
                <text className="text-[11px] text-muted">
                  {t('wifi.eventState', { state: event.state ?? (event.connected === undefined ? '--' : event.connected ? t('wifi.connected') : t('wifi.disconnected')) })}
                </text>
                <text className="text-[11px] text-muted">{`Secure: ${event.secure === undefined ? '--' : event.secure ? 'Yes' : 'No'}`}</text>
              </view>
            ))}
          </view>
        )}
      </Section>
      <Section
        title={t('wifi.scanWifiNetworks')}
        description={t('wifi.getListOfAvailable')}
        action={
          <Button size="sm" testId="wifi-scan" disabled={!!data.scanning} onClick={() => actions.getWifiList()}>
            {data.scanning ? t('wifi.scanning') : t('wifi.scan')}
          </Button>
        }
      >
        {data.wifiList !== null && (
          <text data-testid="wifi-scan-count" className="text-sm font-semibold">{`Available Networks (${networks.length})`}</text>
        )}
        {data.wifiList !== null && !networks.length && <EmptyState icon="device" title={t('wifi.noNetworksFound')} />}
        <scroll className="max-h-96 gap-2">
          {networks.map((wifi, index) => (
            <button
              key={wifi.bssid || `${wifi.ssid}-${index}`}
              data-testid={`wifi-network-${index}`}
              onClick={() => ssid.set(wifi.ssid || '')}
              className="p-3 rounded-lg border border-line bg-surface text-text text-left active:bg-surface-sunken"
            >
              <view className="gap-1">
                <view className="flex-row items-center justify-between gap-2">
                  <text className="grow basis-0 min-w-0 text-sm font-semibold">{wifi.ssid || 'Hidden network'}</text>
                  <text className="px-2 py-1 rounded-full bg-accent-soft text-xs text-accent">{percent(wifi.signalStrength)}</text>
                </view>
                {!!wifi.bssid && <text className="text-xs text-muted">{`BSSID: ${wifi.bssid}`}</text>}
                <text className="text-xs text-muted">{wifi.secure ? 'Security: Secured' : 'Security: Open'}</text>
              </view>
            </button>
          ))}
        </scroll>
      </Section>
      <Section
        title={t('wifi.connectToWifi')}
        description={networks.length ? t('wifi.connectPick') : t('wifi.connectManual')}
      >
        <Field testId="wifi-ssid" label="SSID" bind={ssid} placeholder={networks.length ? 'Enter SSID or select a network above' : 'SSID'} />
        <Field testId="wifi-password" label={t('wifi.password')} bind={password} type="password" placeholder={t('wifi.passwordOptional')} />
        <Button
          testId="wifi-connect"
          disabled={!ssid.value.trim()}
          onClick={() => actions.connectWifi({ ssid: ssid.value.trim(), password: password.value || undefined })}
        >
          {t('wifi.connect')}
        </Button>
      </Section>
    </Page>
  );
}
