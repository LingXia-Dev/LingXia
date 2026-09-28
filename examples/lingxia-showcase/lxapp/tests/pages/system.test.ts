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
      displayLanguage: data?.displayLanguage ?? '',
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
  let system = (await app.page({ name: 'system' }, { timeout: 30_000 })).view;
  await system.testId('system-base-info').waitFor({ timeout: 30_000 });
  await system.testId("system-base-info").click();
  const base = await waitForSystemState(
    app,
    (state) => !!state.appBaseInfo?.os && !!state.appBaseInfo?.productName
      && typeof state.displayLanguage === 'string' && state.displayLanguage.length > 0,
  );
  await expect(system.testId('system-base-result'))
    .toContainText(base.appBaseInfo!.productName!, { timeout: 30_000 });

  // A relaunch replaces the page instance: bind the new one.
  await app.nav.relaunch({ page: 'system', query: { type: 'systemSetting' } });
  system = (await app.page({ name: 'system' }, { timeout: 30_000 })).view;
  await system.testId('system-setting-info').waitFor({ timeout: 30_000 });
  await system.testId("system-setting-info").click();
  await waitForSystemState(
    app,
    (state) => typeof state.systemSetting?.wifiEnabled === 'boolean',
  );
  await expect(system.testId('system-setting-result')).toContainText('WiFi Enabled', { timeout: 30_000 });
});

spec('opens the product cache panel from the rendered API menu', {
  id: 'SYSTEM-CACHE-001',
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
}, async (t) => {
  const { app } = bindFixture(t, 'SYSTEM-CACHE-001');

  await app.nav.relaunch({ page: 'api' });
  const api = (await app.page({ name: 'api' }, { timeout: 30_000 })).view;
  await api.testId('api-system-section').waitFor({ state: 'visible', timeout: 30_000 });
  await api.testId("api-system-section").click();
  await api.testId('api-system-cache').waitFor({ state: 'visible', timeout: 30_000 });
  // The banner demo row sits above this item; the click scrolls it into view
  // first, or the Windows hit lands on chrome / the tab bar and navigation
  // never starts.
  await api.testId("api-system-cache").click();
  const system = (await app.page({ name: 'system' }, { timeout: 30_000 })).view;
  const panel = system.testId('system-cache-panel');
  await panel.waitFor({ state: 'visible', timeout: 30_000 });
  await expect(panel).toContainText('Product Cache');
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
  const system = (await app.page({ name: 'system' }, { timeout: 30_000 })).view;
  await system.testId('system-banner-panel').waitFor({ state: 'visible', timeout: 30_000 });

  // Re-read only refreshes support — seed a false so the click is proven.
  await app.logic.eval(({ getCurrentPages }) => {
    const page = getCurrentPages().find((candidate) => candidate.route.includes('/system/'));
    if (!page) throw new Error('system page missing');
    page.setData({ bannerLast: 'probe-cleared', bannerSupported: false, bannerBusy: false });
    return page.data.bannerLast;
  });

  await system.testId("system-banner-reread").click();
  const reread = await eventually(
    () => bannerPageState(app),
    (state) => state.bannerSupported && state.bannerLast === 'probe-cleared',
    { describe: 'Re-read click to restore support without clobbering Last result' },
  );
  expect(reread.bannerSupported).toBe(true);
  expect(reread.bannerLast).toBe('probe-cleared');

  await system.testId("system-banner-prompt").click();
  await eventually(
    () => bannerPageState(app),
    (state) => state.bannerBusy && state.bannerActiveId === 'showcase-banner-prompt',
    { describe: 'Prompt to park a pending banner.show' },
  );

  // After Prompt the native card is WS_EX_TOPMOST over the WebView. Windows
  // CDP clicks then miss the page button; fire the same control from the DOM.
  await waitForElementEnabled(system, '[data-testid="system-banner-dismiss"]');
  await system.eval(({ document }) => {
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
