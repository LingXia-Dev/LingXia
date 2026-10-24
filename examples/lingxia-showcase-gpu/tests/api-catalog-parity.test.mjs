import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import vm from 'node:vm';
const require = createRequire(new URL('../../../packages/lingxia-ui/package.json', import.meta.url));
const source = require('esbuild').buildSync({
  entryPoints: [fileURLToPath(new URL('../lxapp/pages/api/index.ui.tsx', import.meta.url))],
  bundle: true, write: false, format: 'cjs', external: ['@lingxia/ui'], loader: { '.css': 'empty' },
}).outputFiles[0].text;
const module = { exports: {} };
vm.runInNewContext(source, { module, exports: module.exports, require: () => ({}) });
const groups = JSON.parse(JSON.stringify(module.exports.groups));
const reference = readFileSync(new URL('../../lingxia-showcase/lxapp/pages/api/index.tsx', import.meta.url), 'utf8');

test('API groups retain the React reference order', () => {
  const expected = [...reference.matchAll(/onClick=\{\(\) => toggleSection\(\{ section: '([^']+)' \}\)\}/g)].map(match => match[1]);
  assert.equal(expected.length, 8);
  assert.deepEqual(groups.map(group => group.id), expected);
});

test('every reference API destination has an entry with the same query and order', () => {
  const actions = {
    navigateToTestMiniApp: 'openOtherApp', navigateToCloudPage: 'openCloud',
    navigateToStreamPage: 'openStream', navigateToChannelPage: 'openChannel',
    navigateToBridgeReproPage: 'openBridge', openDeepSeek: 'openDeepSeek',
    navigateToUIPage: 'openUi', navigateToPullDownRefreshPage: 'openRefresh',
    navigateToSystemPage: 'openSystem', exitApp: 'exitApp', navigateToDevicePage: 'openDevice',
    navigateToWifiPage: 'openWifi', navigateToSharePage: 'openShare',
    navigateToClipboardPage: 'openClipboard', navigateToLeaveGuardPage: 'openLeaveGuard',
    navigateToMediaPage: 'openMedia', navigateToOpenFilePage: 'openFile',
    navigateToChooseFilePage: 'openFile', navigateToLocationPage: 'openLocation',
  };
  const expected = [];
  for (const match of reference.matchAll(/onClick=\{(?:\(\) => )?(\w+)(?:\(\{ type: '([^']+)' \}\))?\}/g)) {
    const action = actions[match[1]];
    assert.ok(action, `unmapped reference action ${match[1]}`);
    const payload = match[2] ? { type: match[2] } : match[1] === 'navigateToOpenFilePage' ? { section: 'openFile' } : match[1] === 'navigateToChooseFilePage' ? { section: 'chooseFile' } : null;
    expected.push({ action, payload });
  }
  assert.ok(expected.length > 40, 'reference extraction includes all sections');
  const actual = groups.flatMap(group => group.entries).map(item => ({ action: item.action, payload: item.payload ?? null }));
  actual.push({ action: 'openLocation', payload: null });
  assert.deepEqual(actual, expected);
  const ids = groups.flatMap(group => group.entries.map(item => item.id));
  assert.equal(new Set(ids).size, ids.length, 'entry identities remain unique');
});
