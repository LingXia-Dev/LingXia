import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample, renderPage } from '../testing/harness.mjs';
import { encodeQR } from './encode.ts';

test('Join: an image named by what it is for; the modules are drawn, not read', async () => {
  const screen = await renderExample('QRCode/QRCode', 'Join');
  const code = screen.getByTestId('join');
  assert.deepEqual([code.props.role, code.props.accessibilityLabel, code.props.width], ['img', 'Code to join Heights Guest', 160]);
  const [vector] = code.children;
  assert.equal(vector.props.accessibilityHidden, 1);
  const source = JSON.parse(vector.props.vectorSource);
  assert.deepEqual([source.paths[0].fill, source.paths[1].fill], ['#ffffff', '#000000'], 'dark on light, in either theme');
});

test('the encoder: version, size and the three finder patterns', () => {
  const modules = encodeQR('HELLO', 'H');
  assert.equal(modules.length, 21, 'version 1');
  const finder = (x, y) => [0, 1, 2, 3, 4, 5, 6].every(i => modules[y][x + i] && modules[y + 6][x + i] && modules[y + i][x] && modules[y + i][x + 6]);
  assert.ok(finder(0, 0) && finder(14, 0) && finder(0, 14));
  assert.equal(encodeQR('https://example.com/invite/7Q4M', 'M').length, 29, 'version 3');
});

test('a value past version 15 throws, naming the limit', async () => {
  assert.throws(() => encodeQR('x'.repeat(500), 'M', 15), /do not fit version 15/);
});

test('every path stays inside the View\'s vector limits at the largest version', async () => {
  const screen = await renderPage(`
    import { QRCode } from '@liulidesign/core/QRCode';
    export function render() { return <page><QRCode value={'z'.repeat(400)} label="Long" level="L" data-testid="long" /></page>; }
  `);
  const source = JSON.parse(screen.getByTestId('long').children[0].props.vectorSource);
  assert.ok(source.paths.length <= 32);
  assert.ok(JSON.stringify(source).length <= 32768);
  for (const path of source.paths) assert.ok(path.d.length <= 16384 && (path.d.match(/[MHVhvz]/g) ?? []).length <= 1024);
});

test('the rectangles cover exactly the dark modules, inside the quiet zone', async () => {
  const value = 'https://example.com/invite/7Q4M';
  const screen = await renderPage(`
    import { QRCode } from '@liulidesign/core/QRCode';
    export function render() { return <page><QRCode value="${value}" label="Invite" data-testid="code" /></page>; }
  `);
  const source = JSON.parse(screen.getByTestId('code').children[0].props.vectorSource);
  const modules = encodeQR(value, 'M');
  const size = modules.length;
  assert.deepEqual(source.viewBox, [0, 0, size + 8, size + 8]);
  const drawn = modules.map(row => row.map(() => 0));
  for (const path of source.paths.slice(1)) {
    for (const [, x, y, w, h] of path.d.matchAll(/M(\d+) (\d+)h(\d+)v(\d+)h-\d+z/g)) {
      for (let dy = 0; dy < Number(h); dy++) for (let dx = 0; dx < Number(w); dx++) drawn[Number(y) - 4 + dy][Number(x) - 4 + dx] += 1;
    }
  }
  assert.deepEqual(drawn, modules.map(row => row.map(dark => (dark ? 1 : 0))));
});
