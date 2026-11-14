import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { appearances, format, needed, palette, ratio, roleNames } from '@liulidesign/core/contrast';

// Every contrast pair a doc file lists, computed for the default theme in light and dark.

const catalog = JSON.parse(readFileSync(new URL('../catalog.json', import.meta.url), 'utf8'));
const pairs = catalog.components.flatMap(entry => entry.accessibility.contrast.map(pair => ({ component: entry.name, ...pair })));
const theme = await palette();
const key = (pair, appearance) => `${pair.component}: ${pair.what} (${appearance})`;
const each = () => pairs.flatMap(pair => appearances.map(appearance => ({ pair, appearance, result: ratio(theme.roles[appearance], pair) })));

test('contrast pairs name View roles, with a backdrop under any translucent background', () => {
  assert.ok(pairs.length > 0);
  for (const pair of pairs) {
    for (const spec of [...pair.pair, ...(pair.over ? [pair.over] : [])])
      assert.ok(roleNames.includes(spec.split('/')[0]), `${pair.component}: ${pair.what}: ${spec} is not a View role`);
    assert.equal(pair.pair[1].includes('/'), pair.over !== undefined, `${pair.component}: ${pair.what}: \`over\` goes with a translucent background`);
  }
});

test('the default theme reaches every pair, light and dark', () => {
  const failing = each().flatMap(({ pair, appearance, result }) => {
    if (result.missing) return [`${key(pair, appearance)}: needs ${result.missing.join(', ')}`];
    return result.value < needed(pair) ? [`${key(pair, appearance)}: ${format(result.value)}, needs ${pair.minimum}`] : [];
  });
  assert.deepEqual(failing, []);
});

/** Pairs that clear their minimum by under 0.1: a palette nudge can tip them, so each is named. */
const tight = new Set([]);

test('the tight pairs are named', () => {
  const near = each().filter(({ pair, result }) => !result.missing && result.value >= needed(pair) && result.value - needed(pair) < 0.1).map(({ pair, appearance }) => key(pair, appearance));
  assert.deepEqual(near.sort(), [...tight].sort());
});
