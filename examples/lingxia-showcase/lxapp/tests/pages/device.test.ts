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
  await app.view.testId('device-get-info', { page: 'device' }).waitFor({ timeout: 30_000 });
  await app.view.testId("device-get-info", { page: 'device' }).click();
  const device = await waitForState(app, (state) => !!state.deviceInfo?.osName);
  await app.view.testId('device-info-result', { page: 'device' }).waitFor({ timeout: 30_000 });
  const deviceResult = await app.view.testId('device-info-result', { page: 'device' }).query();
  expect(deviceResult.exists && deviceResult.text).toContain(device.deviceInfo?.osName);

  await app.nav.relaunch({ page: 'device', query: { type: 'screen' } });
  await app.view.testId('device-screen-get-info', { page: 'device' }).waitFor({ timeout: 30_000 });
  await app.view.testId("device-screen-get-info", { page: 'device' }).click();
  const screen = await waitForState(
    app,
    (state) => !!state.screenInfo
      && Number(state.screenInfo.width) > 0
      && Number(state.screenInfo.height) > 0
      && Number(state.screenInfo.scale) > 0,
  );
  await app.view.testId('device-screen-result', { page: 'device' }).waitFor({ timeout: 30_000 });
  expect(Number(screen.screenInfo?.width)).toBeGreaterThan(0);
});

spec("keep network query and listener behavior equivalent across renderers", { id: "DEVICE-002", covers: ['lx.getNetworkInfo'], app: SHOWCASE_APP_ID }, async (t) => {
  const { app } = bindFixture(t, "DEVICE-002");

  for (const type of ['networkType', 'localIP'] as const) {
    await app.nav.relaunch({ page: 'device', query: { type } });
    await app.view.testId('device-network-get-info', { page: 'device' }).waitFor({ timeout: 30_000 });
    await app.view.testId("device-network-get-info", { page: 'device' }).click();
    const state = await waitForState(
      app,
      (candidate) => typeof candidate.networkInfo?.isConnected === 'boolean'
        && !!candidate.networkInfo?.networkType,
    );
    expect(Array.isArray(state.networkInfo?.ipv4)).toBeTruthy();
    expect(Array.isArray(state.networkInfo?.ipv6)).toBeTruthy();
    const result = await app.view.testId('device-network-result', { page: 'device' }).query();
    expect(result.exists && result.text.trim().length > 0).toBeTruthy();
  }

  await app.nav.relaunch({ page: 'device', query: { type: 'networkStatus' } });
  await app.view.testId('device-network-listen-start', { page: 'device' }).waitFor({ timeout: 30_000 });
  await app.view.testId("device-network-listen-start", { page: 'device' }).click();
  await waitForState(app, (state) => state.networkListening);
  await waitForElementText(
    t,
    'device',
    '[data-testid="device-network-status"]',
    (text) => text.includes('Yes'),
    30_000,
  );

  await app.view.testId("device-network-listen-stop", { page: 'device' }).click();
  await waitForState(app, (state) => !state.networkListening);
  await waitForElementText(
    t,
    'device',
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
  await app.view.testId('api-device-section', { page: 'api' }).waitFor({ state: 'visible', timeout: 30_000 });
  await app.view.testId("api-device-section", { page: 'api' }).click();
  await app.view.testId('api-device-section', { page: 'api' }).waitFor({ state: 'visible', timeout: 30_000 });

  const text = await eventually(
    () => app.view.eval({ page: 'api' }, ({ document }) => document.body.innerText ?? ''),
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
