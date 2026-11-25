import { expect, test } from '@lingxia/test';
import homeConfig from '../../../pages/home/index.json';
import uiConfig from '../../../pages/ui/index.json';
import pullConfig from '../../../pages/pulltorefresh/index.json';
import enUS from '../../../i18n/en-US.json';
import zhCN from '../../../i18n/zh-CN.json';
import { SHOWCASE_APP_ID } from '@lingxia/test/showcase/helpers/app.js';
import { androidDevice } from '@lingxia/test/showcase/helpers/android-device.js';
import { bindFixture } from '@lingxia/test/showcase/helpers/poll.js';

const args = globalThis.__LINGXIA_AUTOMATION_HOST__?.args ?? {} as Record<string, string>;
test.configure({ skip: args.platform === 'android' ? undefined : 'Android only' });

test('page JSON controls native navigation chrome and refresh eligibility', {
  id: 'ANDROID-PAGE-CONFIG-001', app: SHOWCASE_APP_ID,
  covers: ['PageConfig.navigationStyle', 'PageConfig.enablePullDownRefresh', 'page.navigationBar.update'],
  requires: { args: ['androidDevice'] }, reason: 'requires Android native hierarchy', timeout: 60000,
}, async (t) => {
  const { app, defer, namespace } = bindFixture(t, 'ANDROID-PAGE-CONFIG-001');
  const device = androidDevice(t);
  defer(async () => { await app.nav.reset({ page: 'home' }); });
  const language = await app.logic.eval(({ lx }) => lx.app.language.get());
  const catalogue = (language.toLowerCase().startsWith('zh') ? zhCN : enUS) as Record<string, { defaultMessage: string }>;
  const title = (config: typeof uiConfig) => catalogue[config.navigationBar.title.message].defaultMessage;
  expect(homeConfig.navigationStyle).toBe('custom');
  expect(pullConfig.enablePullDownRefresh).toBe(true);

  await app.nav.reset({ page: 'ui' });
  // A GPU page's title is its View's `<page title>`; the demo keeps it in page data.
  await app.logic.eval(({ currentPage }) => currentPage()!.update((d) => { (d as { navTitle: string | null }).navTitle = null; }));
  await expect.poll(async () => (await app.info()).navigationBar?.title).toBe(title(uiConfig));
  await expect.poll(async () => (await device.nodes()).some((node) => node.text === title(uiConfig))).toBe(true);
  // Unique runtime titles distinguish native chrome from matching page text.
  const marker = `Navbar ${namespace}`;
  await app.logic.eval(({ currentPage }, title) => currentPage()!.update((d) => { (d as { navTitle: string | null }).navTitle = title; }), marker);
  await expect.poll(async () => (await device.nodes()).some((node) => node.text === marker)).toBe(true);
  const refused = await app.logic.eval(async ({ currentPage }) => {
    try { await currentPage()!.navigationBar.update({ title: 'Other' }); return ''; }
    catch (error) { return (error as { code: string }).code; }
  });
  expect(refused).toBe('E_INVALID_ARG');
  await app.nav.reset({ page: 'home' });

  const disabled = await app.logic.eval(async ({ currentPage }) => {
    try { currentPage()!.refresh(); return { code: '', reason: '' }; }
    catch (error) {
      const value = error as { code: string; reason?: string };
      return { code: value.code, reason: value.reason ?? '' };
    }
  });
  expect(disabled).toEqual({ code: 'E_INVALID_STATE', reason: 'refreshNotEnabled' });

  await app.nav.reset({ page: 'pullToRefresh' });
  await expect.poll(async () => (await app.info()).navigationBar?.title).toBe(title(pullConfig));
  await expect.poll(async () => (await device.nodes()).some((node) => node.text === title(pullConfig))).toBe(true);
  const page = await app.page({ name: 'pullToRefresh' });
  await app.logic.eval(({ currentPage }) => currentPage()!.refresh());
  try {
    await expect(page.view.testId('pull-refresh-count')).toHaveText('1');
    await expect(page.view.testId('pull-refresh-status')).toContainText('Refreshing');
  } finally {
    await app.logic.eval(({ pageStack }) => { const pages = pageStack(); (pages[pages.length - 1].actions as { stopRefresh(): void }).stopRefresh(); });
  }
});
