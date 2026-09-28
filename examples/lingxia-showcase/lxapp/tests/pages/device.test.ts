import type { TestApp } from '@lingxia/test';
import { expect, spec } from '@lingxia/test';
import { waitForElementText } from '../helpers/page.js';
import { bindFixture, eventually, specNamespace } from '../helpers/poll.js';
import { SHOWCASE_APP_ID } from '../helpers/app.js';

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

async function deviceState(app: TestApp): Promise<DevicePageState> {
  return app.logic.eval(({ getCurrentPages }): DevicePageState => {
    const page = getCurrentPages().find((candidate) => candidate.route.includes('/device/'));
    const data = (page?.data ?? {}) as Partial<DevicePageState>;
    return {
      deviceInfo: data.deviceInfo ?? null,
      screenInfo: data.screenInfo ?? null,
      networkInfo: data.networkInfo ?? null,
      networkListening: !!data.networkListening,
    };
  });
}

async function waitForState(
  app: TestApp,
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
  let page = (await app.page({ name: 'device' }, { timeout: 30_000 })).view;
  await page.testId('device-get-info').waitFor({ timeout: 30_000 });
  await page.testId("device-get-info").click();
  const device = await waitForState(app, (state) => !!state.deviceInfo?.osName);
  await expect(page.testId('device-info-result')).toContainText(device.deviceInfo!.osName!, { timeout: 30_000 });

  // A relaunch replaces the page instance: bind the new one.
  await app.nav.relaunch({ page: 'device', query: { type: 'screen' } });
  page = (await app.page({ name: 'device' }, { timeout: 30_000 })).view;
  await page.testId('device-screen-get-info').waitFor({ timeout: 30_000 });
  await page.testId("device-screen-get-info").click();
  const screen = await waitForState(
    app,
    (state) => !!state.screenInfo
      && Number(state.screenInfo.width) > 0
      && Number(state.screenInfo.height) > 0
      && Number(state.screenInfo.scale) > 0,
  );
  await page.testId('device-screen-result').waitFor({ timeout: 30_000 });
  expect(Number(screen.screenInfo?.width)).toBeGreaterThan(0);
});

spec("keep network query and listener behavior equivalent across renderers", { id: "DEVICE-002", covers: ['lx.getNetworkInfo'], app: SHOWCASE_APP_ID }, async (t) => {
  const { app } = bindFixture(t, "DEVICE-002");

  for (const type of ['networkType', 'localIP'] as const) {
    await app.nav.relaunch({ page: 'device', query: { type } });
    const network = (await app.page({ name: 'device' }, { timeout: 30_000 })).view;
    await network.testId('device-network-get-info').waitFor({ timeout: 30_000 });
    await network.testId("device-network-get-info").click();
    const state = await waitForState(
      app,
      (candidate) => typeof candidate.networkInfo?.isConnected === 'boolean'
        && !!candidate.networkInfo?.networkType,
    );
    expect(Array.isArray(state.networkInfo?.ipv4)).toBeTruthy();
    expect(Array.isArray(state.networkInfo?.ipv6)).toBeTruthy();
    expect(await network.testId('device-network-result').textContent()).not.toBe('');
  }

  await app.nav.relaunch({ page: 'device', query: { type: 'networkStatus' } });
  const page = (await app.page({ name: 'device' }, { timeout: 30_000 })).view;
  await page.testId('device-network-listen-start').waitFor({ timeout: 30_000 });
  await page.testId("device-network-listen-start").click();
  await waitForState(app, (state) => state.networkListening);
  await waitForElementText(
    t,
    page,
    '[data-testid="device-network-status"]',
    (text) => text.includes('Yes'),
    30_000,
  );

  await page.testId("device-network-listen-stop").click();
  await waitForState(app, (state) => !state.networkListening);
  await waitForElementText(
    t,
    page,
    '[data-testid="device-network-status"]',
    (text) => text.includes('No'),
    30_000,
  );
});

spec('publishes every device mode in the rendered API menu', {
  timeout: 60_000,
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const app = t.app;
  await app.nav.relaunch({ page: 'api' });
  const api = (await app.page({ name: 'api' }, { timeout: 30_000 })).view;
  await api.testId('api-device-section').waitFor({ state: 'visible', timeout: 30_000 });
  await api.testId("api-device-section").click();
  await api.testId('api-device-section').waitFor({ state: 'visible', timeout: 30_000 });

  const text = await eventually(
    () => api.eval(({ document }) => document.body.innerText ?? ''),
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
