import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample, renderPage } from '../testing/harness.mjs';
import { textOf } from '@lingxia/view/testing';
import { encodeQR, qrRoom } from './encode.ts';

test('Join: an image named by what it is for; the modules are drawn, not read', async () => {
  const screen = await renderExample('QRCode/QRCode', 'Join');
  const code = screen.getByTestId('join');
  assert.deepEqual([code.props.role, code.props.accessibilityLabel, code.props.width], ['img', 'Code to join Harbour Café Guest', 160]);
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

test('fitsQRCode answers at the limit the part draws to', async () => {
  const limits = Object.fromEntries(['L', 'M', 'Q', 'H'].map(level => [level, qrRoom('', level, 15).limit]));
  for (const [level, limit] of Object.entries(limits)) assert.equal(encodeQR('x'.repeat(limit), level, 15).length, 77, `the limit encodes at version 15, level ${level}`);
  const screen = await renderPage(`
    import { fitsQRCode } from '@liulidesign/core/QRCode';
    const limits = ${JSON.stringify(limits)};
    export function render() {
      const answers = Object.entries(limits).map(([level, limit]) => [fitsQRCode('x'.repeat(limit), level), fitsQRCode('x'.repeat(limit + 1), level)]);
      return <page><text data-testid="answers">{JSON.stringify([answers, fitsQRCode('é'.repeat(250))])}</text></page>;
    }
  `);
  const [answers, accented] = JSON.parse(textOf(screen.getByTestId('answers')));
  assert.deepEqual(answers, [[true, false], [true, false], [true, false], [true, false]], 'L, M, Q, H: the limit fits, one more byte does not');
  assert.equal(accented, false, 'bytes, not characters: 250 accented letters are 500 bytes, over the 412 that level M holds');
});

test('a value too long for a code draws an empty tile, still named, and warns once with the numbers', async () => {
  const warned = [];
  const warn = console.warn;
  console.warn = message => warned.push(message);
  try {
    const screen = await renderPage(`
      import { QRCode } from '@liulidesign/core/QRCode';
      export function render() { return <page><QRCode value={'x'.repeat(600)} label="Code for the report" data-testid="long" /><QRCode value={'x'.repeat(600)} label="Code for the report" /></page>; }
    `);
    const tile = screen.getByTestId('long');
    assert.deepEqual([tile.props.role, tile.props.accessibilityLabel, tile.props.width, tile.children.length], ['img', 'Code for the report', 160, 0]);
    const { limit } = qrRoom('', 'M', 15);
    assert.deepEqual(warned.filter(m => m.includes('QRCode')), [`Liuli: QRCode "Code for the report": the value is 600 bytes; at level M a code holds at most ${limit}. It is drawn empty; check with fitsQRCode().`]);
  } finally {
    console.warn = warn;
  }
});

test('the WIFI guidance: its example reads back to the name and password it says', async () => {
  const { doc } = await import('./QRCode.doc.ts');
  const line = doc.guidance.do.find(text => text.includes('WIFI:'));
  for (const char of ['\\', ';', ',', '"', ':']) assert.ok(line.includes('`' + char + '`'), `names ${char}`);
  assert.match(line, /hex digits in double quotes/);
  const value = line.match(/`(WIFI:T:[^`]*)`/)[1];
  // Fields end at an unescaped ';'; a backslash takes the next character as it is.
  const fields = {};
  for (let i = 'WIFI:'.length, key = '', text = '', inKey = true; i < value.length; i++) {
    const char = value[i];
    if (char === '\\') text += value[++i];
    else if (inKey && char === ':') { key = text; text = ''; inKey = false; }
    else if (char === ';') { if (key) fields[key] = text; key = ''; text = ''; inKey = true; }
    else text += char;
  }
  assert.deepEqual(fields, { T: 'WPA', S: 'Harbour Café; Upstairs', P: 'tide:pool,73' });
  assert.ok(line.includes('"Harbour Café; Upstairs"') && line.includes('"tide:pool,73"'), 'the words match the value');
});
