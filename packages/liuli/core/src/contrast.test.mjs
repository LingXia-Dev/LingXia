import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { appearances, format, needed, palette, ratio, roleNames } from '../scripts/contrast.mjs';

// Every contrast pair a doc file lists, computed for the default theme in light and dark.

const catalog = JSON.parse(readFileSync(new URL('../catalog.json', import.meta.url), 'utf8'));
const pairs = catalog.components.flatMap(entry => entry.accessibility.contrast.map(pair => ({ component: entry.name, ...pair })));
const theme = await palette();
const key = (pair, appearance) => `${pair.component}: ${pair.what} (${appearance})`;

/** Pairs the default theme fails today: the input to the theme roles work, not a Liuli fix. */
const themeGaps = new Set([]);

test('contrast pairs name View roles, with a backdrop under any translucent background', () => {
  for (const pair of pairs) {
    for (const spec of [...pair.pair, ...(pair.over ? [pair.over] : [])])
      assert.ok(roleNames.includes(spec.split('/')[0]), `${pair.component}: ${pair.what}: ${spec} is not a View role`);
    assert.equal(pair.pair[1].includes('/'), pair.over !== undefined, `${pair.component}: ${pair.what}: \`over\` goes with a translucent background`);
  }
});

test('the default theme reaches every computable pair, except the listed theme gaps', () => {
  const failing = [];
  for (const pair of pairs)
    for (const appearance of appearances) {
      const result = ratio(theme.roles[appearance], pair);
      if (!result.missing && result.value < needed(pair)) failing.push({ key: key(pair, appearance), line: `${format(result.value)}, needs ${pair.minimum}` });
    }
  const report = failing.map(f => `${f.key}: ${f.line}`).join('\n');
  assert.deepEqual(failing.map(f => f.key).sort(), [...themeGaps].sort(), report);
});

test('pairs on roles the engine derives wait for the resolved palette', t => {
  const waiting = pairs.flatMap(pair => appearances.flatMap(appearance => {
    const result = ratio(theme.roles[appearance], pair);
    return result.missing ? [`${key(pair, appearance)}: needs ${result.missing.join(', ')}`] : [];
  }));
  if (theme.source === '@lingxia/view/theme') assert.deepEqual(waiting, []);
  t.diagnostic(`${waiting.length} pair-appearances wait for the resolved palette (${theme.source})`);
});

/** Pairs that clear their minimum by under 0.1: a palette nudge can tip them, so each is named. */
const tight = new Set(['Item: chevron (light)', 'Toast: action (dark)']);

test('the tight pairs are named and still clear their minimum', () => {
  const near = [];
  for (const pair of pairs)
    for (const appearance of appearances) {
      const result = ratio(theme.roles[appearance], pair);
      if (result.missing) continue;
      if (tight.has(key(pair, appearance))) assert.ok(result.value >= needed(pair), `${key(pair, appearance)}: ${format(result.value)}, needs ${pair.minimum}`);
      if (result.value >= needed(pair) && result.value - needed(pair) < 0.1) near.push(key(pair, appearance));
    }
  assert.deepEqual(near.sort(), [...tight].sort());
});
