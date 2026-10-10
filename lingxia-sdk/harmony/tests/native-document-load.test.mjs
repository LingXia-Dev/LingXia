import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import ts from '../../../packages/node_modules/typescript/lib/typescript.js';

const source = await readFile(new URL('../lingxia/src/main/ets/lxapp/NativeDocumentLoad.ets', import.meta.url), 'utf8');
const { outputText } = ts.transpileModule(source, {
  compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 },
});
const { NativeDocumentLoad } = await import('data:text/javascript;base64,' + Buffer.from(outputText).toString('base64'));
const html = '<!doctype html><p>Farshore 中文 100%</p>';
const base64 = Buffer.from(html).toString('base64');
const encoded = 'data:text/html;charset=utf-8,' + encodeURIComponent(html);
const historyUrl = 'lingxia://newtab/?__lingxia_native_load=test-1';
const pending = () => new NativeDocumentLoad(html, base64, historyUrl);

test('admits the exact native HTML once, including Unicode and percent signs', () => {
  const load = pending();
  assert.equal(load.consume(encoded, true), true);
  assert.equal(load.consume(encoded, true), false);
});
test('admits the equivalent base64 HTML', () => {
  assert.equal(pending().consume('data:text/html;charset=UTF-8;base64,' + base64, true), true);
});
test('a subframe cannot consume the native top-level load', () => {
  const load = pending();
  assert.equal(load.consume(encoded, false), false);
  assert.equal(load.consume(encoded, true), true);
});
test('a different document cannot claim or retain the pending load', () => {
  const load = pending();
  assert.equal(load.consume('data:text/html,' + encodeURIComponent(html + '<script>1</script>'), true), false);
  assert.equal(load.consume(encoded, true), false);
});
test('rejects ordinary URLs, different media types, malformed encoding and parameters', () => {
  for (const url of ['https://example.com', 'data:application/javascript,' + encodeURIComponent(html),
    'data:text/html,%zz', 'data:text/html;charset=latin1,' + encodeURIComponent(html),
    'data:text/html;base64,not-the-native-document']) {
    assert.equal(pending().consume(url, true), false, url);
  }
});

test('only the admitted document begin receives the native history URL once', () => {
  const load = pending();
  assert.equal(load.consume(encoded, true), true);
  assert.equal(load.beginUrl(encoded), historyUrl);
  assert.equal(load.beginUrl(encoded), encoded);
});
test('an unrelated document begin cannot borrow native authority', () => {
  const load = pending();
  assert.equal(load.beginUrl(encoded), encoded);
  assert.equal(load.consume(encoded, true), true);
  assert.equal(load.beginUrl('https://example.com'), 'https://example.com');
  assert.equal(load.beginUrl(encoded), encoded);
});

test('the matching end callback is correlated once after the admitted begin', () => {
  const load = pending();
  assert.equal(load.endUrl(encoded), encoded);
  assert.equal(load.consume(encoded, true), true);
  assert.equal(load.beginUrl(encoded), historyUrl);
  assert.equal(load.endUrl(encoded), historyUrl);
  assert.equal(load.endUrl(encoded), encoded);
});
test('an unrelated end callback cannot borrow native authority', () => {
  const load = pending();
  load.consume(encoded, true);
  load.beginUrl(encoded);
  assert.equal(load.endUrl('https://example.com'), 'https://example.com');
  assert.equal(load.endUrl(encoded), encoded);
});
