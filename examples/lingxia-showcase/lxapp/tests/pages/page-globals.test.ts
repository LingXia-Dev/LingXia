import { expect, spec, type LogicApp, type LogicPage } from '@lingxia/test';
import type { PageSurface } from '@lingxia/types';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { bindFixture } from '../helpers/poll.js';

interface UiPage extends LogicPage<{ logicCounter: number; instanceTag: string; currentType: string }> {
  surface?: PageSurface;
}

spec('keep App identity across pages and merge setData into the correct View', {
  id: 'PAGE-GLOBALS-001', app: SHOWCASE_APP_ID,
  covers: ['getApp', 'getCurrentPages', 'Page.route', 'Page.setData', 'Page.flush', 'lx.navigateTo'],
}, async (t) => {
  const { app, namespace, defer } = bindFixture(t, 'PAGE-GLOBALS-001');
  defer(async () => {
    await app.logic.eval((_, key) => { delete (globalThis as Record<string, unknown>)[key]; }, namespace);
    await app.nav.relaunch({ page: 'home' });
  });
  await app.nav.relaunch({ page: 'ui', query: { type: 'navigation' } });
  const page = await app.page({ name: 'ui' });
  const first = await app.logic.eval(({ getApp, getCurrentPages }, key) => {
    const instance = getApp();
    if (!instance) throw new Error('App instance missing');
    (globalThis as Record<string, unknown>)[key] = instance;
    const pages = getCurrentPages<UiPage>();
    const current = pages[pages.length - 1];
    return { route: current.route, surfaceAbsent: current.surface === undefined, tag: current.data.instanceTag };
  }, namespace);
  expect(first.route.replace(/\.(tsx|vue)$/, '')).toBe('pages/ui/index');
  expect(first.surfaceAbsent).toBe(true);
  expect(first.tag.length > 0).toBe(true);
  await page.view.testId('lifecycle-bump-logic').click();
  await expect(page.view.testId('lifecycle-logic-counter')).toHaveText('1');
  // The contract here is setData/flush itself, including two writes in one turn.
  const merged = await app.logic.eval(async ({ getPage }, id) => {
    const current = getPage<UiPage>(id);
    if (!current) throw new Error('Page instance missing');
    current.setData({ logicCounter: 7 });
    current.setData({ logicCounter: current.data.logicCounter + 1 });
    await current.flush();
    return { counter: current.data.logicCounter, tag: current.data.instanceTag, type: current.data.currentType };
  }, page.instanceId);
  expect(merged).toEqual({ counter: 8, tag: first.tag, type: 'navigation' });
  await expect(page.view.testId('lifecycle-logic-counter')).toHaveText('8');
  await app.nav.to({ page: 'feedback' });
  const next = await app.logic.eval(({ getApp, getCurrentPages }, key) => {
    const pages = getCurrentPages();
    return {
      sameApp: getApp<LogicApp>() === (globalThis as Record<string, unknown>)[key],
      routes: pages.map(page => page.route),
    };
  }, namespace);
  expect(next.sameApp).toBe(true);
  expect(next.routes.map(route => route.replace(/\.(tsx|vue)$/, ''))).toEqual(['pages/ui/index', 'pages/feedback/index']);
  await app.nav.back();
  await expect(page.view.testId('lifecycle-logic-counter')).toHaveText('8');
});

spec('inject the owning surface into a page opened as a float', {
  id: 'PAGE-GLOBALS-002', app: SHOWCASE_APP_ID,
  covers: ['Page.surface', 'Page.route', 'lx.surface.openPage', 'PageSurface.id', 'PageSurface.close'],
}, async (t) => {
  const { app, namespace, defer } = bindFixture(t, 'PAGE-GLOBALS-002');
  defer(async () => {
    await app.logic.eval(async ({ lx }, key) => {
      const surface = lx.surface.getByKey(key);
      if (surface?.kind === 'page') await surface.close();
    }, namespace);
    await app.nav.relaunch({ page: 'home' });
  });
  await app.nav.relaunch({ page: 'home' });
  const opened = await app.logic.eval(async ({ lx }, key) => {
    const surface = await lx.surface.openPage('surface', { as: 'float', key, query: { fixture: key } });
    return { id: surface.id, key: surface.key ?? null };
  }, namespace);
  const page = await app.page({ name: 'surface' });
  await expect(page.view.testId('surface-page')).toBeVisible();
  const owner = await app.logic.eval(({ getPage }, id) => {
    const page = getPage<UiPage>(id);
    if (!page?.surface) throw new Error('Page surface missing');
    return { route: page.route, id: page.surface.id, key: page.surface.key ?? null, alive: page.surface.alive };
  }, page.instanceId);
  expect({ ...owner, route: owner.route.replace(/\.(tsx|vue)$/, '') }).toEqual({ route: 'pages/surface/index', id: opened.id, key: namespace, alive: true });
  expect(opened.key).toBe(namespace);
  await app.logic.eval(async ({ getPage }, id) => {
    const surface = getPage<UiPage>(id)?.surface;
    if (!surface) throw new Error('Page surface missing');
    await surface.close();
  }, page.instanceId);
  await expect.poll(() => app.logic.eval(({ lx }, key) => lx.surface.getByKey(key) == null, namespace)).toBe(true);
  await expect((await app.page({ name: 'home' })).view.testId('home-page')).toBeVisible();
});
