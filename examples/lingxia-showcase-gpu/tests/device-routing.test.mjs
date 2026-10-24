import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import vm from 'node:vm';
const require = createRequire(new URL('../../../packages/lingxia-ui/package.json', import.meta.url));
const source = require('esbuild').buildSync({ entryPoints: [fileURLToPath(new URL('../lxapp/pages/device/index.ts', import.meta.url))], bundle: true, write: false, format: 'iife' }).outputFiles[0].text;
function fixture() {
  let page, listener, stops = 0;
  const lx = { onNetworkChange(callback) { listener = callback; return () => { stops++; listener = null; }; } };
  vm.runInNewContext(source, { console: { log() {} }, lx, Page(definition) { page = { ...definition, setData(patch) { Object.assign(this.data, patch); } }; } });
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
