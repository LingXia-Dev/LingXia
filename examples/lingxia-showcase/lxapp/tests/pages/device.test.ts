import { expect, test } from '@rongjs/test';
import type { LxAppDriver } from 'lingxia-types';

interface DevicePageState {
  deviceInfo: { osName?: string } | null;
  screenInfo: { width?: number; height?: number; scale?: number } | null;
  networkInfo: {
    isConnected?: boolean;
    networkType?: string;
    ipv4?: string[];
    ipv6?: string[];
  } | null;
  networkListening: boolean;
}

async function deviceState(app: LxAppDriver): Promise<DevicePageState> {
  return app.eval({
    script: `
      const page = getCurrentPages().find((candidate) => candidate.route.includes('/device/'));
      return {
        deviceInfo: page?.data?.deviceInfo ?? null,
        screenInfo: page?.data?.screenInfo ?? null,
        networkInfo: page?.data?.networkInfo ?? null,
        networkListening: !!page?.data?.networkListening,
      };
    `,
  }) as Promise<DevicePageState>;
}

async function waitForState(
  app: LxAppDriver,
  predicate: (state: DevicePageState) => boolean,
): Promise<DevicePageState> {
  const deadline = Date.now() + 10_000;
  let state = await deviceState(app);
  while (Date.now() < deadline) {
    if (predicate(state)) return state;
    await new Promise((resolve) => setTimeout(resolve, 50));
    state = await deviceState(app);
  }
  throw new Error(`Timed out waiting for device page state: ${JSON.stringify(state)}`);
}

async function waitForElementText(
  app: LxAppDriver,
  css: string,
  text: string,
): Promise<void> {
  const deadline = Date.now() + 10_000;
  while (Date.now() < deadline) {
    const element = await app.page.query({ page: 'device', css, full: true });
    if (element.exists && element.text.includes(text)) return;
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw new Error(`Timed out waiting for '${css}' to contain '${text}'`);
}

test('renders device and screen API results after real UI actions', async () => {
  const app = lx.automation().lxapp();

  await app.nav.relaunch({ page: 'device', query: { type: 'device' } });
  await app.page.waitFor({ page: 'device', css: '[data-testid="device-get-info"]' });
  await app.page.click({ page: 'device', css: '[data-testid="device-get-info"]' });
  const device = await waitForState(app, (state) => !!state.deviceInfo?.osName);
  await app.page.waitFor({ page: 'device', css: '[data-testid="device-info-result"]' });
  const deviceResult = await app.page.query({
    page: 'device',
    css: '[data-testid="device-info-result"]',
    full: true,
  });
  expect(deviceResult.exists && deviceResult.text).toContain(device.deviceInfo?.osName);

  await app.nav.relaunch({ page: 'device', query: { type: 'screen' } });
  await app.page.waitFor({ page: 'device', css: '[data-testid="device-screen-get-info"]' });
  await app.page.click({ page: 'device', css: '[data-testid="device-screen-get-info"]' });
  const screen = await waitForState(
    app,
    (state) => !!state.screenInfo
      && Number(state.screenInfo.width) > 0
      && Number(state.screenInfo.height) > 0
      && Number(state.screenInfo.scale) > 0,
  );
  await app.page.waitFor({ page: 'device', css: '[data-testid="device-screen-result"]' });
  expect(Number(screen.screenInfo?.width) > 0).toBeTruthy();
});

test('keeps React and Vue network modes behaviorally equivalent', async () => {
  const app = lx.automation().lxapp();

  for (const type of ['networkType', 'localIP'] as const) {
    await app.nav.relaunch({ page: 'device', query: { type } });
    await app.page.waitFor({ page: 'device', css: '[data-testid="device-network-get-info"]' });
    await app.page.click({ page: 'device', css: '[data-testid="device-network-get-info"]' });
    const state = await waitForState(
      app,
      (candidate) => typeof candidate.networkInfo?.isConnected === 'boolean'
        && !!candidate.networkInfo?.networkType,
    );
    expect(Array.isArray(state.networkInfo?.ipv4)).toBeTruthy();
    expect(Array.isArray(state.networkInfo?.ipv6)).toBeTruthy();
    const result = await app.page.query({
      page: 'device',
      css: '[data-testid="device-network-result"]',
      full: true,
    });
    expect(result.exists && result.text.trim().length > 0).toBeTruthy();
  }

  await app.nav.relaunch({ page: 'device', query: { type: 'networkStatus' } });
  await app.page.waitFor({ page: 'device', css: '[data-testid="device-network-listen-start"]' });
  await app.page.click({ page: 'device', css: '[data-testid="device-network-listen-start"]' });
  await waitForState(app, (state) => state.networkListening);
  await waitForElementText(app, '[data-testid="device-network-status"]', 'Yes');

  await app.page.click({ page: 'device', css: '[data-testid="device-network-listen-stop"]' });
  await waitForState(app, (state) => !state.networkListening);
  await waitForElementText(app, '[data-testid="device-network-status"]', 'No');
});

test('publishes every device mode in the rendered API menu', async () => {
  const app = lx.automation().lxapp();
  await app.nav.relaunch({ page: 'api' });
  await app.page.waitFor({ page: 'api', css: '[data-testid="api-device-section"]' });
  await app.page.click({ page: 'api', css: '[data-testid="api-device-section"]' });

  const text = await app.page.eval({
    page: 'api',
    script: 'document.body.innerText',
  }) as string;
  for (const label of [
    'Device Info',
    'Screen Info',
    'Vibration',
    'Phone Call',
    'Device Orientation',
    'Network Type',
    'Local IP Address',
    'Network Status Listener',
    'WiFi',
  ]) {
    expect(text).toContain(label);
  }
});
