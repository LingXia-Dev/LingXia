import type { TestApp } from '@lingxia/test';
import { expect, spec } from '@lingxia/test';
import { bindFixture, eventually, specNamespace } from '../helpers/poll.js';
import { waitForElementEnabled } from '../helpers/page.js';
import { SHOWCASE_APP_ID } from '../helpers/app.js';

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
  return app.eval({
    script: `
      const page = getCurrentPages().find((candidate) => candidate.route.includes('/system/'));
      return {
        appBaseInfo: page?.data?.appBaseInfo ?? null,
        displayLanguage: page?.data?.displayLanguage ?? null,
        systemSetting: page?.data?.systemSetting ?? null,
      };
    `,
  }) as Promise<SystemPageState>;
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

spec("render host app and system information through page actions", { id: "SYSTEM-001", covers: ['lx.app.getBaseInfo', 'lx.app.displayLanguage.get', 'lx.getSystemSetting'], app: SHOWCASE_APP_ID }, async (t) => {
  const { app } = bindFixture(t, "SYSTEM-001");


  await app.nav.relaunch({ page: 'system', query: { type: 'appBaseInfo' } });
  await app.page.waitFor({ page: 'system', css: '[data-testid="system-base-info"]' });
  await app.page.testId("system-base-info", { page: 'system' }).click();
  const base = await waitForSystemState(
    app,
    (state) => !!state.appBaseInfo?.os && !!state.appBaseInfo?.productName
      && typeof state.displayLanguage === 'string' && state.displayLanguage.length > 0,
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
  await app.page.testId("system-setting-info", { page: 'system' }).click();
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

spec('opens the product cache panel from the rendered API menu', {
  id: 'SYSTEM-CACHE-001',
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
}, async (t) => {
  const { app } = bindFixture(t, 'SYSTEM-CACHE-001');

  await app.nav.relaunch({ page: 'api' });
  await app.page.waitFor({
    page: 'api',
    css: '[data-testid="api-system-section"]',
    state: 'visible',
  });
  await app.page.testId("api-system-section", { page: 'api' }).click();
  await app.page.waitFor({
    page: 'api',
    css: '[data-testid="api-system-cache"]',
    state: 'visible',
  });
  // The banner demo row sits above this item; without a scroll the Windows
  // hit lands on chrome / the tab bar and navigation never starts.
  await app.page.scrollTo({ page: 'api', css: '[data-testid="api-system-cache"]' });
  await app.page.testId("api-system-cache", { page: 'api' }).click();
  await app.page.waitFor({
    page: 'system',
    css: '[data-testid="system-cache-panel"]',
    state: 'visible',
  });

  const panel = await app.page.query({
    page: 'system',
    css: '[data-testid="system-cache-panel"]',
    full: true,
  });
  expect(panel.exists && panel.text).toContain('Product Cache');
});

async function bannerPageState(app: TestApp): Promise<BannerPageState> {
  return app.eval({
    script: `
      const page = getCurrentPages().find((candidate) => candidate.route.includes('/system/'));
      return {
        bannerLast: page?.data?.bannerLast ?? '',
        bannerBusy: !!page?.data?.bannerBusy,
        bannerActiveId: page?.data?.bannerActiveId ?? '',
        bannerSupported: !!page?.data?.bannerSupported,
      };
    `,
  }) as Promise<BannerPageState>;
}

bannerPageSpec('drive banner re-read, prompt, and dismiss from the system page', {
  id: 'SYSTEM-BANNER-001',
  covers: ['lx.app.banner', 'lx.app.banner.show', 'lx.app.banner.dismiss'],
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
  reason: 'Desktop banner is Control-app / macOS / Windows only.',
}, async (t) => {
  const { app } = bindFixture(t, 'SYSTEM-BANNER-001');
  t.defer(async () => {
    await app.eval({
      script: `try {
        await lx.app.banner.dismiss('showcase-banner-toast');
        await lx.app.banner.dismiss('showcase-banner-prompt');
      } catch {}
      return true;`,
    });
  });

  await app.nav.relaunch({ page: 'system', query: { type: 'banner' } });
  await app.page.waitFor({
    page: 'system',
    css: '[data-testid="system-banner-panel"]',
    state: 'visible',
  });

  // Re-read only refreshes support — seed a false so the click is proven.
  await app.eval({
    script: `
      const page = getCurrentPages().find((candidate) => candidate.route.includes('/system/'));
      if (!page) throw new Error('system page missing');
      page.setData({ bannerLast: 'probe-cleared', bannerSupported: false, bannerBusy: false });
      return page.data.bannerLast;
    `,
  });

  await app.page.testId("system-banner-reread", { page: 'system' }).click();
  const reread = await eventually(
    () => bannerPageState(app),
    (state) => state.bannerSupported && state.bannerLast === 'probe-cleared',
    { describe: 'Re-read click to restore support without clobbering Last result' },
  );
  expect(reread.bannerSupported).toBe(true);
  expect(reread.bannerLast).toBe('probe-cleared');

  await app.page.testId("system-banner-prompt", { page: 'system' }).click();
  await eventually(
    () => bannerPageState(app),
    (state) => state.bannerBusy && state.bannerActiveId === 'showcase-banner-prompt',
    { describe: 'Prompt to park a pending banner.show' },
  );

  // After Prompt the native card is WS_EX_TOPMOST over the WebView. Windows
  // CDP clicks then miss the page button; fire the same control from the DOM.
  await waitForElementEnabled(app, 'system', '[data-testid="system-banner-dismiss"]');
  await app.page.eval({
    page: 'system',
    script: `
      const button = document.querySelector('[data-testid="system-banner-dismiss"]');
      if (!(button instanceof HTMLButtonElement) || button.disabled) {
        throw new Error('dismiss is not clickable');
      }
      button.click();
    `,
  });
  const dismissed = await eventually(
    () => bannerPageState(app),
    (state) => !state.bannerBusy && state.bannerLast === 'dismissed',
    { describe: 'Dismiss to resolve the prompt as dismissed' },
  );
  expect(dismissed.bannerLast).toBe('dismissed');
});
