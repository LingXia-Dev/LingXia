import { expect, spec } from '@lingxia/test';
import type { LxAppDriver } from 'lingxia-types/automation';
import { waitForElementText } from '../helpers/page.js';
import { bindFixture, eventually, specNamespace } from '../helpers/poll.js';
import { showcaseApp, SHOWCASE_APP_ID } from '../helpers/app.js';

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
  return eventually(deviceState.bind(null, app), predicate, {
    describe: 'device page state',
    timeoutMs: 30_000,
  });
}

spec("render device and screen API results after real UI actions", { id: "DEVICE-001", covers: ['lx.getDeviceInfo', 'lx.getScreenInfo'], app: SHOWCASE_APP_ID }, async (t) => {
  const { app } = bindFixture(t, "DEVICE-001");


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

spec("keep network query and listener behavior equivalent across renderers", { id: "DEVICE-002", covers: ['lx.getNetworkInfo'], app: SHOWCASE_APP_ID }, async (t) => {
  const { app } = bindFixture(t, "DEVICE-002");


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
  await waitForElementText(
    app,
    'device',
    '[data-testid="device-network-status"]',
    (text) => text.includes('Yes'),
    30_000,
  );

  await app.page.click({ page: 'device', css: '[data-testid="device-network-listen-stop"]' });
  await waitForState(app, (state) => !state.networkListening);
  await waitForElementText(
    app,
    'device',
    '[data-testid="device-network-status"]',
    (text) => text.includes('No'),
    30_000,
  );
});

spec('publishes every device mode in the rendered API menu', {
  timeout: 60_000,
}, async () => {
  const app = showcaseApp();
  await app.nav.relaunch({ page: 'api' });
  await app.page.waitFor({
    page: 'api',
    css: '[data-testid="api-device-section"]',
    state: 'visible',
  });
  await app.page.click({ page: 'api', css: '[data-testid="api-device-section"]' });
  await app.page.waitFor({
    page: 'api',
    css: '[data-testid="api-device-section"]',
    state: 'visible',
  });

  const text = await eventually(
    () => app.page.eval({
      page: 'api',
      script: 'document.body.innerText',
    }) as Promise<string>,
    (body) => [
      'Device Info',
      'Screen Info',
      'Vibration',
      'Phone Call',
      'Device Orientation',
      'Network Type',
      'Local IP Address',
      'Network Status Listener',
      'WiFi',
    ].every((label) => body.includes(label)),
    { describe: 'API page device-mode labels', timeoutMs: 15_000 },
  );
  expect(text.includes('Device Info')).toBeTruthy();
});
