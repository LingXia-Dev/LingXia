import { expect, test } from '@lingxia/test';
import uiConfig from '../pages/ui/index.json';
import enUS from '../i18n/en-US.json';
import zhCN from '../i18n/zh-CN.json';
import { SHOWCASE_APP_ID } from '@lingxia/test/showcase/helpers/app.js';
import { bindFixture } from '@lingxia/test/showcase/helpers/poll.js';

test('a GPU page\'s <page title> is its navigation bar title; Logic cannot set it', {
  id: 'PAGE-TITLE-001',
  app: SHOWCASE_APP_ID,
  covers: ['page.title', 'page.navigationBar.update'],
}, async (t) => {
  const { app, defer, namespace } = bindFixture(t, 'PAGE-TITLE-001');
  defer(async () => { await app.nav.reset({ page: 'home' }); });
  const language = await app.logic.eval(({ lx }) => lx.app.language.get());
  const catalogue = (language.toLowerCase().startsWith('zh') ? zhCN : enUS) as Record<string, { defaultMessage: string }>;
  const configured = catalogue[uiConfig.navigationBar.title.message].defaultMessage;
  const setTitle = (title: string | null) => app.logic.eval(({ currentPage }, title) =>
    currentPage()!.update((d) => { (d as { navTitle: string | null }).navTitle = title; }), title);

  await app.nav.reset({ page: 'ui' });
  const marker = `Title ${namespace}`;
  await setTitle(marker);
  await expect.poll(async () => (await app.info()).navigationBar?.title).toBe(marker);
  await setTitle(null);
  await expect.poll(async () => (await app.info()).navigationBar?.title).toBe(configured);

  const refused = await app.logic.eval(async ({ currentPage }) => {
    try { await currentPage()!.navigationBar.update({ title: 'Other' }); return ''; }
    catch (error) { return (error as { code: string }).code; }
  });
  expect(refused).toBe('E_INVALID_ARG');
  expect((await app.info()).navigationBar?.title).toBe(configured);
});
