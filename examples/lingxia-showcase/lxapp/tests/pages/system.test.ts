import { expect } from '@rongjs/test';
import type { LxAppDriver } from 'lingxia-types/automation';
import { contract, eventually } from '../support/contract.js';

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
  return eventually(systemState.bind(null, app), predicate, {
    describe: 'system page state',
    timeoutMs: 30_000,
  });
}

contract({
  id: 'SYSTEM-001',
  title: 'render host app and system information through page actions',
  covers: ['lx.app.getBaseInfo', 'lx.getSystemSetting'],
  layer: 'logic',
  levels: ['semantic', 'boundary'],
  scope: 'portable',
  expectedOutcome: 'supported',
}, async ({ app }) => {

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
