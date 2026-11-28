import { test } from 'node:test';
import assert from 'node:assert/strict';
import { bundlePage, mountPage } from './page-harness.mjs';
const source = bundlePage('../lxapp/pages/api/index.ts');
function fixture({ ui = {}, openApp, ...overrides } = {}) {
  let page;
  const calls = [];
  const record = name => async (...args) => { calls.push([name, ...structuredClone(args)]); };
  // A page target returns a handle; every other target a promise.
  const go = target => {
    if (target.app) return openApp ? openApp(target) : record('app')(target);
    calls.push(['navigate', structuredClone(target)]);
    const opened = Promise.resolve();
    return { opened, result: new Promise(() => {}) };
  };
  const lx = {
    nav: { go, back: record('back') },
    ui: { toast: record('toast'), ...ui },
    openExternal: record('external'), surface: { open: ({ url }, ...options) => record('url')(url, ...options) }, host: { exit: record('exit') },
    ...overrides,
  };
  page = mountPage(source, { lx });
  return { page, calls };
}
test('cloud availability requires both provider APIs', () => {
  for (const [apis, expected] of [[{}, false], [{ cloud: {} }, false], [{ auth: {} }, false], [{ cloud: {}, auth: {} }, true]]) {
    const { page } = fixture(apis); page.onLoad(); assert.equal(page.data.cloudAvailable, expected);
  }
});
test('file entries select the requested demo', async () => {
  const { page, calls } = fixture();
  await page.openFile(); await page.openFile({ section: 'chooseFile' });
  assert.deepEqual(calls, [['navigate', { page: 'file', params: { section: 'openFile' } }], ['navigate', { page: 'file', params: { section: 'chooseFile' } }]]);
});
test('website destinations dispatch independently and cancel does nothing', async () => {
  for (const id of ['self', 'aside', 'external', null]) {
    const { page, calls } = fixture({ ui: { actionSheet: async () => id ? { status: 'selected', id } : { status: 'canceled' } } });
    await page.openDeepSeek();
    const url = 'https://www.deepseek.com/';
    assert.deepEqual(calls, id === null ? [] : id === 'external' ? [['external', url]] : id === 'aside' ? [['url', url, { as: 'aside' }]] : [['url', url]]);
  }
});
test('failed external launches surface an actionable error', async () => {
  const { page, calls } = fixture({ ui: { actionSheet: async () => ({ status: 'selected', id: 'external' }) }, openExternal: async () => { throw { message: 'Browser unavailable' }; } });
  await page.openDeepSeek();
  assert.deepEqual(calls, [['toast', { title: 'Browser unavailable', icon: 'none' }]]);
});
test('other app opens chat and launch errors are shown', async () => {
  const ok = fixture(); await ok.page.openOtherApp();
  assert.deepEqual(ok.calls, [['app', { app: 'lingxia-showcase-web', page: 'chat' }]]);
  const failed = fixture({ openApp: async () => { throw { message: 'App unavailable' }; } });
  await failed.page.openOtherApp();
  assert.equal(failed.calls[0][1].title, 'App unavailable');
});
test('exit requires accepting the native confirmation', async () => {
  for (const status of ['canceled', 'confirmed']) {
    const { page, calls } = fixture({ ui: { confirm: async () => status === 'confirmed' } });
    await page.exitApp(); assert.deepEqual(calls, status === 'canceled' ? [] : [['exit']]);
  }
});
