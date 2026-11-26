import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { appearances, format, looks, needed, palette, ratio, roleNames, roleUnder, surfaces } from '../scripts/contrast.mjs';

// Every contrast pair a doc file lists, computed for the default theme in light and dark.

const catalog = JSON.parse(readFileSync(new URL('../catalog.json', import.meta.url), 'utf8'));
const pairs = catalog.components.flatMap(entry => entry.accessibility.contrast.map(pair => ({ component: entry.name, ...pair })));
const theme = await palette();
// Translucent by an opacity step (`text/6`) or by a role the theme leaves translucent (`neutral-soft`, `media`).
const isTranslucent = spec => spec.includes('/') || Object.values(theme.roles).some(roles => roles[spec]?.length === 9 && !roles[spec].endsWith('ff'));
const key = (pair, appearance) => `${pair.component}: ${pair.what} (${appearance})`;

/** Pairs the default theme fails today: the input to the theme roles work, not a Liuli fix. */
const themeGaps = new Set([]);

test('contrast pairs name View roles, with a backdrop under an opacity step, and only under a translucent background', () => {
  for (const pair of pairs) {
    for (const spec of [...pair.pair, ...(pair.over ? [pair.over] : [])])
      assert.ok(roleNames.includes(spec.split('/')[0]), `${pair.component}: ${pair.what}: ${spec} is not a View role`);
    if (pair.pair[1].includes('/')) assert.ok(pair.over !== undefined, `${pair.component}: ${pair.what}: a translucent background needs \`over\``);
    if (pair.over !== undefined) assert.ok(isTranslucent(pair.pair[1]), `${pair.component}: ${pair.what}: \`over\` goes with a translucent background`);
  }
});

test('the default theme reaches every computable pair, except the listed theme gaps', () => {
  assert.equal(theme.source, '@lingxia/view/theme', 'run npm install in packages/: the resolved palette is missing');
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

/** High contrast holds text to 7:1 and a 3:1 mark to 4.5:1, as the theme boosts its own pairs. */
const boosted = pair => (needed(pair) >= 4.5 ? 7 : 4.5);

/** Pairs the high-contrast looks miss today: the theme's accessibility work, named so none is added. */
const highContrastGaps = new Set([
  'Avatar: initials (high-contrast-light)',
  'Banner: action on neutral (high-contrast-light)',
  'Banner: action on info (high-contrast-light)',
  'Banner: action on success (high-contrast-light)',
  'Banner: action on warning (high-contrast-light)',
  'Banner: action on danger (high-contrast-light)',
  'Button: disabled label on its 6% ink fill, on the page (high-contrast-light)',
  'Button: disabled label on its 6% ink fill, on a card (high-contrast-light)',
  'Code: title and line numbers (high-contrast-light)',
  'Code: a sign and number on an added line (high-contrast-light)',
  'Code: a sign and number on a removed line (high-contrast-light)',
  'Code: a number on a marked line (high-contrast-light)',
  'Code: a hunk or file header on its tint, over the block (high-contrast-light)',
  'Code: a keyword (high-contrast-light)',
  'Code: a string (high-contrast-light)',
  'Code: a number or literal (high-contrast-light)',
  'Item: second line and end values, selected (high-contrast-light)',
  'MessageBubble: failure words (high-contrast-light)',
  'NavList: current row count (high-contrast-light)',
  'SegmentedControl: other labels (high-contrast-light)',
  'SegmentedControl: thumb edge, with icon-only segments (high-contrast-light)',
  'Slider: thumb rim against the track (high-contrast-light)',
  "Tabs: other segments' text (high-contrast-light)",
  'ToggleButton: disabled label, off, on a card (high-contrast-light)',
  'ToggleButton: disabled label, on (high-contrast-light)',
  'VideoControls: progress against the unplayed track (high-contrast-light)',
  'Avatar: initials (high-contrast-dark)',
  'Banner: action on neutral (high-contrast-dark)',
  'Banner: action on info (high-contrast-dark)',
  'Banner: action on success (high-contrast-dark)',
  'Banner: action on warning (high-contrast-dark)',
  'Banner: action on danger (high-contrast-dark)',
  'Code: a sign and number on an added line (high-contrast-dark)',
  'Code: a sign and number on a removed line (high-contrast-dark)',
  'Code: a hunk or file header on its tint, over the block (high-contrast-dark)',
  'Code: a keyword (high-contrast-dark)',
  'Code: a number or literal (high-contrast-dark)',
  'FileInput: failure line (high-contrast-dark)',
  // Muted fixed text on its sunken box: 6.7:1, the field's own adornment on surface is 7.6:1.
  'InputGroup: the fixed text (high-contrast-light)',
  'MessageBubble: failure words (high-contrast-dark)',
  'Slider: thumb rim against the track (high-contrast-dark)',
  'Toast: action (high-contrast-dark)',
  'VideoControls: progress against the unplayed track (high-contrast-dark)',
]);

test('the high-contrast looks reach every computable pair at the boosted minimum, except the listed gaps', () => {
  assert.equal(theme.source, '@lingxia/view/theme', 'the resolved palette, which carries the high-contrast looks');
  const failing = [];
  for (const pair of pairs)
    for (const look of ['high-contrast-light', 'high-contrast-dark']) {
      const result = ratio(theme.roles[look], pair);
      if (!result.missing && result.value < boosted(pair)) failing.push({ key: key(pair, look), line: `${format(result.value)}, needs ${boosted(pair)}:1` });
    }
  const report = failing.map(f => `${f.key}: ${f.line}`).join('\n');
  assert.deepEqual(failing.map(f => f.key).sort(), [...highContrastGaps].sort(), report);
});

/** Pairs that clear their minimum by under 0.1: a palette nudge can tip them, so each is named. */
const tight = new Set(['Code: a sign and number on a removed line (light)', 'Item: chevron (light)', 'SegmentedControl: thumb edge, with icon-only segments (light)', 'Toast: action (dark)']);

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

/**
 * Each line role, as drawn on each surface in each look: dark keeps `line` at 1.3:1 and `line-strong` at 3:1;
 * high contrast keeps both at 3:1; light keeps the floors the contract sets, which it always met.
 */
const lineFloor = (look, role, surface) => {
  if (look.startsWith('high-contrast')) return 3;
  if (role === 'line-strong') return 3;
  if (look === 'dark') return 1.3;
  return { canvas: 1.1, surface: 1.2, 'surface-sunken': 1.1, 'surface-raised': 1.1, 'surface-overlay': 1.2 }[surface];
};

test('every line role keeps its contrast on every surface in every look, as drawn there', () => {
  assert.equal(theme.source, '@lingxia/view/theme', 'the resolved palette, which carries the per-surface lines');
  const short = [];
  for (const look of looks)
    for (const role of ['line', 'line-strong'])
      for (const surface of surfaces) {
        const result = ratio(theme.roles[look], { pair: [role, surface] });
        if (result.value < lineFloor(look, role, surface)) short.push(`${role} on ${surface} (${look}, drawn as ${roleUnder(role, surface)}): ${format(result.value)}, needs ${lineFloor(look, role, surface)}`);
      }
  assert.deepEqual(short, []);
});
