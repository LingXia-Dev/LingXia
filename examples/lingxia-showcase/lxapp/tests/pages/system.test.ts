import type { TestApp } from '@lingxia/test';
import { expect, spec } from '@lingxia/test';
import { bindFixture, eventually, specNamespace } from '../helpers/poll.js';
import { waitForElementEnabled } from '../helpers/page.js';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import type { ProbeElement } from '../helpers/view.js';

const testArgs = globalThis.__LINGXIA_AUTOMATION_HOST__?.args
  ?? {} as Record<string, string>;
const bannerPageSpec = new Set(['macos', 'windows']).has(
  testArgs.platform?.toLocaleLowerCase() ?? '',
)
  ? spec
  : spec.skip;

interface SystemPageState {
  appBaseInfo: { os?: string; productName?: string } | null;
  displayLanguage?: string;
  systemSetting: { wifiEnabled?: boolean } | null;
}

interface BannerPageState {
  bannerLast: string;
  bannerBusy: boolean;
  bannerActiveId: string;
  bannerSupported: boolean;
}

async function systemState(app: TestApp): Promise<SystemPageState> {
  return app.logic.eval(({ getCurrentPages }) => {
    const page = getCurrentPages().find((candidate) => candidate.route.includes('/system/'));
    const data = page?.data as Partial<SystemPageState> | undefined;
    return {
      appBaseInfo: data?.appBaseInfo ?? null,
      displayLanguage: data?.displayLanguage ?? undefined,
      systemSetting: data?.systemSetting ?? null,
    };
  });
}

async function waitForSystemState(
  app: TestApp,
  predicate: (state: SystemPageState) => boolean,
): Promise<SystemPageState> {
  return eventually(systemState.bind(null, app), predicate, {
    describe: 'system page state',
    timeoutMs: 30_000,
  });
}

spec("render host app and system information through page actions", { id: "SYSTEM-001", covers: ['lx.host.getBaseInfo', 'lx.host.displayLanguage.get', 'lx.getSystemSetting'], app: SHOWCASE_APP_ID }, async (t) => {
  const { app } = bindFixture(t, "SYSTEM-001");


  await app.nav.relaunch({ page: 'system', query: { type: 'appBaseInfo' } });
  await app.view.testId('system-base-info', { page: 'system' }).waitFor({ timeout: 30_000 });
  await app.view.testId("system-base-info", { page: 'system' }).click();
  const base = await waitForSystemState(
    app,
    (state) => !!state.appBaseInfo?.os && !!state.appBaseInfo?.productName
      && typeof state.displayLanguage === 'string' && state.displayLanguage.length > 0,
  );
  const baseResultLocator = app.view.testId('system-base-result', { page: 'system' });
  await baseResultLocator.waitFor({ timeout: 30_000 });
  const baseResult = await baseResultLocator.query();
  expect(baseResult.exists && baseResult.text).toContain(base.appBaseInfo?.productName);

  await app.nav.relaunch({ page: 'system', query: { type: 'systemSetting' } });
  await app.view.testId('system-setting-info', { page: 'system' }).waitFor({ timeout: 30_000 });
  await app.view.testId("system-setting-info", { page: 'system' }).click();
  await waitForSystemState(
    app,
    (state) => typeof state.systemSetting?.wifiEnabled === 'boolean',
  );
  const settingResultLocator = app.view.testId('system-setting-result', { page: 'system' });
  await settingResultLocator.waitFor({ timeout: 30_000 });
  const settingResult = await settingResultLocator.query();
  expect(settingResult.exists && settingResult.text).toContain('WiFi Enabled');
});

spec('opens the product cache panel from the rendered API menu', {
  id: 'SYSTEM-CACHE-001',
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
}, async (t) => {
  const { app } = bindFixture(t, 'SYSTEM-CACHE-001');

  await app.nav.relaunch({ page: 'api' });
  await app.view.testId('api-system-section', { page: 'api' }).waitFor({ state: 'visible', timeout: 30_000 });
  await app.view.testId("api-system-section", { page: 'api' }).click();
  await app.view.testId('api-system-cache', { page: 'api' }).waitFor({ state: 'visible', timeout: 30_000 });
  // The banner demo row sits above this item; the click scrolls it into view
  // first, or the Windows hit lands on chrome / the tab bar and navigation
  // never starts.
  await app.view.testId("api-system-cache", { page: 'api' }).click();
  const panelLocator = app.view.testId('system-cache-panel', { page: 'system' });
  await panelLocator.waitFor({ state: 'visible', timeout: 30_000 });

  const panel = await panelLocator.query();
  expect(panel.exists && panel.text).toContain('Product Cache');
});

async function bannerPageState(app: TestApp): Promise<BannerPageState> {
  return app.logic.eval(({ getCurrentPages }) => {
    const page = getCurrentPages().find((candidate) => candidate.route.includes('/system/'));
    const data = page?.data as Partial<BannerPageState> | undefined;
    return {
      bannerLast: data?.bannerLast ?? '',
      bannerBusy: !!data?.bannerBusy,
      bannerActiveId: data?.bannerActiveId ?? '',
      bannerSupported: !!data?.bannerSupported,
    };
  });
}

bannerPageSpec('drive banner re-read, prompt, and dismiss from the system page', {
  id: 'SYSTEM-BANNER-001',
  covers: ['lx.host.banner', 'lx.host.banner.show', 'lx.host.banner.dismiss'],
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
  reason: 'Desktop banner is Control-app / macOS / Windows only.',
}, async (t) => {
  const { app } = bindFixture(t, 'SYSTEM-BANNER-001');
  t.defer(async () => {
    await app.logic.eval(async ({ lx }) => {
      try {
        await lx.host.banner?.dismiss('showcase-banner-toast');
        await lx.host.banner?.dismiss('showcase-banner-prompt');
      } catch {}
      return true;
    });
  });

  await app.nav.relaunch({ page: 'system', query: { type: 'banner' } });
  await app.view.testId('system-banner-panel', { page: 'system' }).waitFor({ state: 'visible', timeout: 30_000 });

  // Re-read only refreshes support — seed a false so the click is proven.
  await app.logic.eval(({ getCurrentPages }) => {
    const page = getCurrentPages().find((candidate) => candidate.route.includes('/system/'));
    if (!page) throw new Error('system page missing');
    page.setData({ bannerLast: 'probe-cleared', bannerSupported: false, bannerBusy: false });
    return page.data.bannerLast;
  });

  await app.view.testId("system-banner-reread", { page: 'system' }).click();
  const reread = await eventually(
    () => bannerPageState(app),
    (state) => state.bannerSupported && state.bannerLast === 'probe-cleared',
    { describe: 'Re-read click to restore support without clobbering Last result' },
  );
  expect(reread.bannerSupported).toBe(true);
  expect(reread.bannerLast).toBe('probe-cleared');

  await app.view.testId("system-banner-prompt", { page: 'system' }).click();
  await eventually(
    () => bannerPageState(app),
    (state) => state.bannerBusy && state.bannerActiveId === 'showcase-banner-prompt',
    { describe: 'Prompt to park a pending banner.show' },
  );

  // After Prompt the native card is WS_EX_TOPMOST over the WebView. Windows
  // CDP clicks then miss the page button; fire the same control from the DOM.
  await waitForElementEnabled(t, 'system', '[data-testid="system-banner-dismiss"]');
  await app.view.eval({ page: 'system' }, ({ document }) => {
    const button = document.querySelector('[data-testid="system-banner-dismiss"]') as ProbeElement | null;
    if (!button || button.tagName !== 'BUTTON' || button.disabled) {
      throw new Error('dismiss is not clickable');
    }
    button.click();
  });
  const dismissed = await eventually(
    () => bannerPageState(app),
    (state) => !state.bannerBusy && state.bannerLast === 'dismissed',
    { describe: 'Dismiss to resolve the prompt as dismissed' },
  );
  expect(dismissed.bannerLast).toBe('dismissed');
});
