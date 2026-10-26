import { useLocal, type View } from '@lingxia/ui';
import '../../theme/index.css';
import { Button, Field, InfoList, InfoRow, Page, Section } from '../../shared/ui';

const titles: Record<string, string> = {
  device: 'Device information',
  screen: 'Screen information',
  vibrate: 'Device vibration',
  dial: 'Phone call',
  networkType: 'Network type',
  localIP: 'Local IP addresses',
  networkStatus: 'Network status listener',
  orientation: 'Device orientation',
};
const yesNo = (value: unknown) => (value === undefined ? '--' : value ? 'Yes' : 'No');
const number = (value: unknown, suffix = '') =>
  typeof value === 'number' && Number.isFinite(value) ? `${Number.isInteger(value) ? value : value.toFixed(2)}${suffix}` : '--';

export function render({ data, actions }: View) {
  const phone = useLocal('phone', '');
  const requested = String(data.currentType ?? 'device');
  const type = titles[requested] ? requested : 'device';
  const info = (data.deviceInfo ?? {}) as Record<string, unknown>;
  const screen = (data.screenInfo ?? {}) as Record<string, unknown>;
  const network = (data.networkInfo ?? {}) as Record<string, unknown>;
  const changed = (data.networkChange ?? {}) as Record<string, unknown>;
  const events = (data.orientationEvents ?? []) as string[];
  return (
    <Page testId="device-page">
      <text testId="device-mode-title" className="text-2xl font-light text-center">
        {titles[type]}
      </text>
      {type === 'device' && (
        <Section
          title="Get device information"
          description="Brand, model and OS version"
          action={
            <Button testId="device-get-info" onClick={() => actions.getDeviceInfo()}>
              Get info
            </Button>
          }
        >
          {!!data.deviceInfo && (
            <InfoList testId="device-info">
              <InfoRow label="Brand" value={info.brand} />
              <InfoRow label="Market name" value={info.marketName || info.model} />
              <InfoRow label="Model" value={info.model} testId="device-info-model" />
              <InfoRow label="OS name" value={info.osName} testId="device-info-osName" />
              <InfoRow label="OS version" value={info.osVersion} />
            </InfoList>
          )}
        </Section>
      )}
      {type === 'screen' && (
        <Section
          title="Get screen information"
          description="Screen dimensions and scale"
          action={
            <Button testId="device-screen-get-info" onClick={() => actions.getScreenInfo()}>
              Get info
            </Button>
          }
        >
          {!!data.screenInfo && (
            <InfoList testId="device-screen">
              <InfoRow label="Width" value={number(screen.width, 'px')} testId="device-screen-width" />
              <InfoRow label="Height" value={number(screen.height, 'px')} />
              <InfoRow label="Scale" value={number(screen.scale)} testId="device-screen-scale" />
            </InfoList>
          )}
        </Section>
      )}
      {type === 'networkType' && (
        <Section
          title="Get network type"
          description="Wi-Fi, mobile data, Ethernet or offline"
          action={
            <Button testId="device-network-get-info" onClick={() => actions.getNetworkInfo()}>
              Get info
            </Button>
          }
        >
          <InfoList testId="device-network">
            <InfoRow label="Connected" value={yesNo(network.isConnected)} />
            <InfoRow label="Network type" value={network.networkType} testId="device-network-type" />
          </InfoList>
        </Section>
      )}
      {type === 'localIP' && (
        <Section
          title="Get local IP addresses"
          description="IPv4 and IPv6 on the active network"
          action={
            <Button testId="device-network-get-info" onClick={() => actions.getNetworkInfo()}>
              Get IP
            </Button>
          }
        >
          <InfoList testId="device-network">
            <InfoRow label="IPv4" value={network.ipv4} testId="device-ipv4" />
            <InfoRow label="IPv6" value={network.ipv6} testId="device-ipv6" />
          </InfoList>
        </Section>
      )}
      {type === 'networkStatus' && (
        <Section title="Network status listener" description="Report connectivity changes as they happen">
          <grid className="gap-3 grid-cols-2">
            <Button testId="device-network-listen-start" onClick={() => actions.startNetworkChangeListen()}>
              Start listening
            </Button>
            <Button variant="secondary" testId="device-network-listen-stop" onClick={() => actions.stopNetworkChangeListen()}>
              Stop listening
            </Button>
          </grid>
          <InfoList testId="device-network-status">
            <InfoRow label="Listening" value={data.networkListening ? 'Yes' : 'No'} testId="device-network-listening" />
            <InfoRow label="Connected" value={yesNo(changed.isConnected)} />
            <InfoRow label="Network type" value={changed.networkType} />
            <InfoRow label="IPv4" value={changed.ipv4} />
            <InfoRow label="IPv6" value={changed.ipv6} />
          </InfoList>
        </Section>
      )}
      {type === 'vibrate' && (
        <Section title="Trigger vibration" description="Try a short or long vibration">
          <grid className="gap-3 grid-cols-2">
            <Button testId="device-vibrate-short" onClick={() => actions.vibrateShort()}>
              Short
            </Button>
            <Button variant="secondary" testId="device-vibrate-long" onClick={() => actions.vibrateLong()}>
              Long
            </Button>
          </grid>
        </Section>
      )}
      {type === 'dial' && (
        <Section title="Dial a phone number" description="Open the native phone dialer">
          <Field testId="device-phone-number" label="Phone number" bind={phone} keyboard="phone" placeholder="Enter phone number" />
          <Button
            testId="device-phone-call"
            disabled={!phone.value.trim()}
            onClick={() => actions.makePhoneCall({ phoneNumber: phone.value.trim() })}
          >
            Call
          </Button>
        </Section>
      )}
      {type === 'orientation' && (
        <Section title="Lock and follow orientation" description="Choose an orientation and listen for changes">
          <grid className="gap-3 grid-cols-2">
            <Button testId="device-orientation-portrait" onClick={() => actions.setOrientationPortrait()}>
              Lock portrait
            </Button>
            <Button testId="device-orientation-landscape" onClick={() => actions.setOrientationLandscape()}>
              Lock landscape
            </Button>
            <Button variant="secondary" testId="device-orientation-listen-start" onClick={() => actions.startDeviceOrientationListen()}>
              Start listening
            </Button>
            <Button variant="secondary" testId="device-orientation-listen-stop" onClick={() => actions.stopDeviceOrientationListen()}>
              Stop listening
            </Button>
          </grid>
          <InfoList>
            <InfoRow label="Listening" value={data.orientationListening ? 'Yes' : 'No'} />
            <InfoRow label="Lock target" value={data.orientationLock} testId="device-orientation-lock" />
            <InfoRow label="Current value" value={data.deviceOrientationValue} testId="device-orientation-value" />
          </InfoList>
          <row className="items-center justify-between gap-3">
            <text className="text-sm font-semibold">Orientation events</text>
            <Button variant="ghost" size="sm" testId="device-orientation-clear" onClick={() => actions.clearOrientationEvents()}>
              Clear logs
            </Button>
          </row>
          <scroll testId="device-orientation-event-scroll" className="max-h-56 p-3 rounded-lg border border-border bg-muted">
            <text testId="device-orientation-events" className="text-xs break-all whitespace-pre-line">
              {events.join('\n') || '--'}
            </text>
          </scroll>
        </Section>
      )}
    </Page>
  );
}
