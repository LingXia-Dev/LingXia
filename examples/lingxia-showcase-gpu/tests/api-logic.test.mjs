import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import vm from 'node:vm';
const require = createRequire(new URL('../../../packages/lingxia-ui/package.json', import.meta.url));
const source = require('esbuild').buildSync({ entryPoints: [fileURLToPath(new URL('../lxapp/pages/api/index.ts', import.meta.url))], bundle: true, write: false, format: 'iife' }).outputFiles[0].text;
function fixture(overrides = {}) {
  let page;
  const calls = [];
  const record = name => async (...args) => { calls.push([name, ...structuredClone(args)]); };
  const lx = { navigateTo: record('navigate'), navigateToApp: record('app'), openExternal: record('external'), surface: { openUrl: record('url') }, host: { exit: record('exit') }, showToast: record('toast'), ...overrides };
  vm.runInNewContext(source, { lx, Page(definition) { page = { ...definition, setData(patch) { Object.assign(this.data, patch); } }; } });
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
  assert.deepEqual(calls, [['navigate', { page: 'file', query: { section: 'openFile' } }], ['navigate', { page: 'file', query: { section: 'chooseFile' } }]]);
});
test('website destinations dispatch independently and cancel does nothing', async () => {
  for (const id of ['self', 'aside', 'external', null]) {
    const { page, calls } = fixture({ showActionSheet: async () => id ? { status: 'selected', id } : { status: 'canceled' } });
    await page.openDeepSeek();
    const url = 'https://www.deepseek.com/';
    assert.deepEqual(calls, id === null ? [] : id === 'external' ? [['external', url]] : id === 'aside' ? [['url', url, { as: 'aside' }]] : [['url', url]]);
  }
});
test('failed external launches surface an actionable error', async () => {
  const { page, calls } = fixture({ showActionSheet: async () => ({ status: 'selected', id: 'external' }), openExternal: async () => { throw { message: 'Browser unavailable' }; } });
  await page.openDeepSeek();
  assert.deepEqual(calls, [['toast', { title: 'Browser unavailable', icon: 'none' }]]);
});
test('other app opens chat and launch errors are shown', async () => {
  const ok = fixture(); await ok.page.openOtherApp();
  assert.deepEqual(ok.calls, [['app', { appId: 'lingxia-chat', page: 'chat' }]]);
  const failed = fixture({ navigateToApp: async () => { throw { message: 'App unavailable' }; } });
  await failed.page.openOtherApp();
  assert.equal(failed.calls[0][1].title, 'App unavailable');
});
test('exit requires accepting the native confirmation', async () => {
  for (const status of ['canceled', 'confirmed']) {
    const { page, calls } = fixture({ showModal: async () => ({ status }) });
    await page.exitApp(); assert.deepEqual(calls, status === 'canceled' ? [] : [['exit']]);
  }
});
