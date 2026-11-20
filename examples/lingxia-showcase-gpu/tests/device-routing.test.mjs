import { test } from 'node:test';
import assert from 'node:assert/strict';
import { bundlePage, mountPage } from './page-harness.mjs';
const source = bundlePage('../lxapp/pages/device/index.ts');
function fixture() {
  let page, listener, stops = 0;
  const lx = { onNetworkChange(callback) { listener = callback; return () => { stops++; listener = null; }; } };
  page = mountPage(source, { console: { log() {} }, lx });
  return { page, get stops() { return stops; }, emit(value) { listener?.(value); } };
}
test('reference Device queries retain their distinct modes', async () => {
  const { page } = fixture();
  for (const type of ['device', 'screen', 'vibrate', 'dial', 'networkType', 'localIP', 'networkStatus', 'orientation']) {
    await page.onLoad({ type }); assert.equal(page.data.currentType, type);
  }
});
test('network listeners subscribe once and are released on hide and unload', () => {
  const f = fixture(); f.page.startNetworkChangeListen(); f.page.startNetworkChangeListen();
  f.emit({ networkType: 'wifi' }); assert.equal(f.page.data.networkChange.networkType, 'wifi');
  f.page.onHide(); assert.equal(f.stops, 1); assert.equal(f.page.data.networkListening, false);
  f.emit({ networkType: 'none' }); assert.equal(f.page.data.networkChange.networkType, 'wifi');
  f.page.startNetworkChangeListen(); f.page.onUnload(); assert.equal(f.stops, 2); assert.equal(f.page.data.networkListening, false);
});
