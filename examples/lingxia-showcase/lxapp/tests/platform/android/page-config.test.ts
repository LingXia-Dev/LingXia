import { expect, spec } from '@lingxia/test';
import homeConfig from '../../../pages/home/index.json';
import uiConfig from '../../../pages/ui/index.json';
import pullConfig from '../../../pages/pulltorefresh/index.json';
import { SHOWCASE_APP_ID } from '../../helpers/app.js';
import { androidDevice } from '../../helpers/android-device.js';
import { bindFixture } from '../../helpers/poll.js';

const args = globalThis.__LINGXIA_AUTOMATION_HOST__?.args ?? {} as Record<string, string>;
const androidSpec = args.platform === 'android' ? spec : spec.skip;

androidSpec('page JSON controls native navigation chrome and refresh eligibility', {
  id: 'ANDROID-PAGE-CONFIG-001', app: SHOWCASE_APP_ID,
  covers: ['PageConfig.navigationStyle', 'PageConfig.enablePullDownRefresh', 'lx.navigationBar.update'],
  requires: { args: ['androidDevice'] }, reason: 'requires Android native hierarchy', timeout: 60000,
}, async (t) => {
  const { app, defer, namespace } = bindFixture(t, 'ANDROID-PAGE-CONFIG-001');
  const device = androidDevice(t);
  defer(async () => { await app.nav.relaunch({ page: 'home' }); });
  const language = await app.logic.eval(({ lx }) => lx.host.displayLanguage.get());
  const title = (config: typeof uiConfig) => language.toLowerCase().split(/[-_]/)[0] === 'zh'
    ? config.navigationBar.title['zh-CN'] : config.navigationBar.title.default;
  expect(homeConfig.navigationStyle).toBe('custom');
  expect(pullConfig.enablePullDownRefresh).toBe(true);

  await app.nav.relaunch({ page: 'ui' });
  // The demo sets a runtime title onLoad; null must restore the JSON title.
  await app.logic.eval(({ lx }) => lx.navigationBar.update({ title: null }));
  await expect.poll(async () => (await app.info()).navigationBar?.title).toBe(title(uiConfig));
  await expect.poll(async () => (await device.nodes()).some((node) => node.text === title(uiConfig))).toBe(true);
  // Unique runtime titles distinguish native chrome from matching page text.
  const marker = `Navbar ${namespace}`;
  await app.logic.eval(({ lx }, title) => lx.navigationBar.update({ title }), marker);
  await expect.poll(async () => (await device.nodes()).some((node) => node.text === marker)).toBe(true);

  await app.nav.relaunch({ page: 'home' });
  await app.logic.eval(({ lx }, title) => lx.navigationBar.update({ title }), marker);
  expect((await app.info()).navigationBar?.title).toBe(marker);
  expect((await device.nodes()).some((node) => node.text === marker)).toBe(false);

  const disabled = await app.logic.eval(async ({ lx }) => {
    try { await lx.startPullDownRefresh(); return { code: '', bizCode: 0 }; }
    catch (error) {
      const value = error as { code: string; data?: { bizCode?: number } };
      return { code: value.code, bizCode: value.data?.bizCode ?? 0 };
    }
  });
  expect(disabled).toEqual({ code: 'E_INVALID_STATE', bizCode: 4004 });

  await app.nav.relaunch({ page: 'pullToRefresh' });
  await expect.poll(async () => (await app.info()).navigationBar?.title).toBe(title(pullConfig));
  await expect.poll(async () => (await device.nodes()).some((node) => node.text === title(pullConfig))).toBe(true);
  const page = await app.page({ name: 'pullToRefresh' });
  await app.logic.eval(({ lx }) => lx.startPullDownRefresh());
  try {
    await expect(page.view.testId('pull-refresh-count')).toHaveText('1');
    await expect(page.view.testId('pull-refresh-status')).toContainText('Refreshing');
  } finally {
    await app.logic.eval(({ lx }) => lx.stopPullDownRefresh());
  }
});
