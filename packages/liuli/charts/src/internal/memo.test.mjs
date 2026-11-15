import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { transform } from 'esbuild';

const { code } = await transform(readFileSync(new URL('./memo.ts', import.meta.url), 'utf8'), { loader: 'ts', format: 'esm' });
const { remember } = await import(`data:text/javascript;base64,${Buffer.from(code).toString('base64')}`);

test('remember builds once per data array and key, and again when the key changes', () => {
  const rows = [{ v: 1 }];
  let built = 0;
  const build = () => { built += 1; return { n: built }; };
  const first = remember(rows, 'bars', 'a', build);
  assert.equal(remember(rows, 'bars', 'a', build), first, 'the same object comes back, as after a theme change');
  assert.equal(built, 1);
  assert.notEqual(remember(rows, 'bars', 'b', build), first, 'a new encoding or choice rebuilds');
  assert.equal(built, 2);
  remember(rows, 'lines', 'b', build);
  assert.equal(built, 3, 'each kind of geometry keeps its own');
  assert.equal(remember(rows, 'bars', 'b', build).n, 2);
  remember([{ v: 1 }], 'bars', 'b', build);
  assert.equal(built, 4, 'new rows rebuild, even when equal');
});
