import { expect, test } from '@rongjs/test';
import type { LxAppDriver } from 'lingxia-types';

interface SystemPageState {
  appBaseInfo: { os?: string; productName?: string } | null;
  systemSetting: { wifiEnabled?: boolean } | null;
}

async function systemState(app: LxAppDriver): Promise<SystemPageState> {
  return app.eval({
    script: `
      const page = getCurrentPages().find((candidate) => candidate.route.includes('/system/'));
      return {
        appBaseInfo: page?.data?.appBaseInfo ?? null,
        systemSetting: page?.data?.systemSetting ?? null,
      };
    `,
  }) as Promise<SystemPageState>;
}

async function waitForSystemState(
  app: LxAppDriver,
  predicate: (state: SystemPageState) => boolean,
): Promise<SystemPageState> {
  const deadline = Date.now() + 10_000;
  let state = await systemState(app);
  while (Date.now() < deadline) {
    if (predicate(state)) return state;
    await new Promise((resolve) => setTimeout(resolve, 50));
    state = await systemState(app);
  }
  throw new Error(`Timed out waiting for system page state: ${JSON.stringify(state)}`);
}

test('renders app and system information through page actions', async () => {
  const app = lx.automation().lxapp();

  await app.nav.relaunch({ page: 'system', query: { type: 'appBaseInfo' } });
  await app.page.waitFor({ page: 'system', css: '[data-testid="system-base-info"]' });
  await app.page.click({ page: 'system', css: '[data-testid="system-base-info"]' });
  const base = await waitForSystemState(
    app,
    (state) => !!state.appBaseInfo?.os && !!state.appBaseInfo?.productName,
  );
  await app.page.waitFor({ page: 'system', css: '[data-testid="system-base-result"]' });
  const baseResult = await app.page.query({
    page: 'system',
    css: '[data-testid="system-base-result"]',
    full: true,
  });
  expect(baseResult.exists && baseResult.text).toContain(base.appBaseInfo?.productName);

  await app.nav.relaunch({ page: 'system', query: { type: 'systemSetting' } });
  await app.page.waitFor({ page: 'system', css: '[data-testid="system-setting-info"]' });
  await app.page.click({ page: 'system', css: '[data-testid="system-setting-info"]' });
  await waitForSystemState(
    app,
    (state) => typeof state.systemSetting?.wifiEnabled === 'boolean',
  );
  await app.page.waitFor({ page: 'system', css: '[data-testid="system-setting-result"]' });
  const settingResult = await app.page.query({
    page: 'system',
    css: '[data-testid="system-setting-result"]',
    full: true,
  });
  expect(settingResult.exists && settingResult.text).toContain('WiFi Enabled');
});
