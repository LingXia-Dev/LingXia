import { t, useLocal, type MessageKey } from '@lingxia/view';
import '../../theme/index.css';
import { Button, Field, InfoList, InfoRow, Page, Section } from '../../shared/ui';
import type { View } from './$types';

const titles = ['device', 'screen', 'vibrate', 'dial', 'networkType', 'localIP', 'networkStatus', 'orientation'];
const yesNo = (value: unknown) => (value === undefined ? '--' : value ? t('common.yes') : t('common.no'));
const number = (value: unknown, suffix = '') =>
  typeof value === 'number' && Number.isFinite(value) ? `${Number.isInteger(value) ? value : value.toFixed(2)}${suffix}` : '--';

export function render({ data, actions }: View) {
  const phone = useLocal('phone', '');
  const requested = String(data.currentType ?? 'device');
  const type = titles.includes(requested) ? requested : 'device';
  const info = (data.deviceInfo ?? {}) as Record<string, unknown>;
  const screen = (data.screenInfo ?? {}) as Record<string, unknown>;
  const network = (data.networkInfo ?? {}) as Record<string, unknown>;
  const changed = (data.networkChange ?? {}) as Record<string, unknown>;
  const events = (data.orientationEvents ?? []) as string[];
  return (
    <Page testId="device-page" data={{ 'data-mode': type }}>
      <text data-testid="device-mode-title" className="text-2xl font-light text-center">
        {t(`device.title.${type}` as MessageKey)}
      </text>
      {type === 'device' && (
        <Section
          title={t('device.getDeviceInformation')}
          description={t('device.brandModelAndOs')}
          action={
            <Button testId="device-get-info" onClick={() => actions.getDeviceInfo()}>
              {t('device.getInfo')}
            </Button>
          }
        >
          {!!data.deviceInfo && (
            <InfoList testId="device-info-result">
              <InfoRow label={t('device.brand')} value={info.brand} />
              <InfoRow label={t('device.marketName')} value={info.marketName || info.model} />
              <InfoRow label={t('device.model')} value={info.model} testId="device-info-model" />
              <InfoRow label={t('device.osName')} value={info.osName} testId="device-info-osName" />
              <InfoRow label={t('device.osVersion')} value={info.osVersion} />
            </InfoList>
          )}
        </Section>
      )}
      {type === 'screen' && (
        <Section
          title={t('device.getScreenInformation')}
          description={t('device.screenDimensionsAndScale')}
          action={
            <Button testId="device-screen-get-info" onClick={() => actions.getScreenInfo()}>
              {t('device.getInfo')}
            </Button>
          }
        >
          {!!data.screenInfo && (
            <InfoList testId="device-screen-result">
              <InfoRow label={t('device.width')} value={number(screen.width, 'px')} testId="device-screen-width" />
              <InfoRow label={t('device.height')} value={number(screen.height, 'px')} />
              <InfoRow label={t('device.scale')} value={number(screen.scale)} testId="device-screen-scale" />
            </InfoList>
          )}
        </Section>
      )}
      {type === 'networkType' && (
        <Section
          title={t('device.getNetworkType')}
          description={t('device.wiFiMobileData')}
          action={
            <Button testId="device-network-get-info" onClick={() => actions.getNetworkInfo()}>
              {t('device.getInfo')}
            </Button>
          }
        >
          <InfoList testId="device-network-result">
            <InfoRow label={t('device.connected')} value={yesNo(network.isConnected)} />
            <InfoRow label={t('device.networkType')} value={network.networkType} testId="device-network-type" />
          </InfoList>
        </Section>
      )}
      {type === 'localIP' && (
        <Section
          title={t('device.getLocalIpAddresses')}
          description={t('device.ipv4AndIpv6On')}
          action={
            <Button testId="device-network-get-info" onClick={() => actions.getNetworkInfo()}>
              {t('device.getIp')}
            </Button>
          }
        >
          <InfoList testId="device-network-result">
            <InfoRow label="IPv4" value={network.ipv4} testId="device-ipv4" />
            <InfoRow label="IPv6" value={network.ipv6} testId="device-ipv6" />
          </InfoList>
        </Section>
      )}
      {type === 'networkStatus' && (
        <Section title={t('device.networkStatusListener')} description={t('device.reportConnectivityChangesAs')}>
          <view className="grid gap-3 grid-cols-2">
            <Button testId="device-network-listen-start" onClick={() => actions.startNetworkChangeListen()}>
              {t('device.startListening')}
            </Button>
            <Button variant="secondary" testId="device-network-listen-stop" onClick={() => actions.stopNetworkChangeListen()}>
              {t('device.stopListening')}
            </Button>
          </view>
          <InfoList testId="device-network-status">
            <InfoRow label={t('device.listening')} value={yesNo(Boolean(data.networkListening))} testId="device-network-listening" />
            <InfoRow label={t('device.connected')} value={yesNo(changed.isConnected)} />
            <InfoRow label={t('device.networkType')} value={changed.networkType} />
            <InfoRow label="IPv4" value={changed.ipv4} />
            <InfoRow label="IPv6" value={changed.ipv6} />
          </InfoList>
        </Section>
      )}
      {type === 'vibrate' && (
        <Section title={t('device.triggerVibration')} description={t('device.tryAShortOr')}>
          <view className="grid gap-3 grid-cols-2">
            <Button testId="device-vibrate-short" onClick={() => actions.vibrateShort()}>
              {t('device.short')}
            </Button>
            <Button variant="secondary" testId="device-vibrate-long" onClick={() => actions.vibrateLong()}>
              {t('device.long')}
            </Button>
          </view>
        </Section>
      )}
      {type === 'dial' && (
        <Section title={t('device.dialAPhoneNumber')} description={t('device.openTheNativePhone')}>
          <Field testId="device-phone-number" label={t('device.phoneNumber')} bind={phone} type="tel" placeholder={t('device.enterPhoneNumber')} />
          <Button
            testId="device-phone-call"
            disabled={!phone.value.trim()}
            onClick={() => actions.makePhoneCall({ phoneNumber: phone.value.trim() })}
          >
            {t('device.call')}
          </Button>
        </Section>
      )}
      {type === 'orientation' && (
        <Section title={t('device.lockAndFollowOrientation')} description={t('device.chooseAnOrientationAnd')}>
          <view className="grid gap-3 grid-cols-2">
            <Button testId="device-orientation-portrait" onClick={() => actions.setOrientationPortrait()}>
              {t('device.lockPortrait')}
            </Button>
            <Button testId="device-orientation-landscape" onClick={() => actions.setOrientationLandscape()}>
              {t('device.lockLandscape')}
            </Button>
            <Button variant="secondary" testId="device-orientation-listen-start" onClick={() => actions.startDeviceOrientationListen()}>
              {t('device.startListening')}
            </Button>
            <Button variant="secondary" testId="device-orientation-listen-stop" onClick={() => actions.stopDeviceOrientationListen()}>
              {t('device.stopListening')}
            </Button>
          </view>
          <InfoList>
            <InfoRow label={t('device.listening')} value={yesNo(Boolean(data.orientationListening))} />
            <InfoRow label={t('device.lockTarget')} value={data.orientationLock} testId="device-orientation-lock" />
            <InfoRow label={t('device.currentValue')} value={data.deviceOrientationValue} testId="device-orientation-value" />
          </InfoList>
          <view className="flex-row items-center justify-between gap-3">
            <text className="text-sm font-semibold">{t('device.orientationEvents')}</text>
            <Button variant="ghost" size="sm" testId="device-orientation-clear" onClick={() => actions.clearOrientationEvents()}>
              {t('device.clearLogs')}
            </Button>
          </view>
          <scroll data-testid="device-orientation-event-scroll" className="max-h-56 p-3 rounded-lg border border-line bg-surface-sunken">
            <text data-testid="device-orientation-events" className="text-xs break-all whitespace-pre-line">
              {events.join('\n') || '--'}
            </text>
          </scroll>
        </Section>
      )}
    </Page>
  );
}
