import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { compileUtilities } from '../../../../lingxia-view/compiler/tailwind.mjs';
import { scales } from '../../../../lingxia-view/src/generated/contract.js';

// Liuli names steps of the theme's ramps; the values are the pack's, decided at run time.
const css = readFileSync(new URL('../../tokens.css', import.meta.url), 'utf8');
const names = JSON.parse(readFileSync(new URL('../../tokens.names.json', import.meta.url), 'utf8'));
const pack = JSON.parse(readFileSync(new URL('../../../../../crates/lingxia-theme/packs/default.json', import.meta.url), 'utf8'));
const token = name => {
  const found = css.match(new RegExp(`^\\s*--${name}:\\s*([^;]+);`, 'm'));
  assert.ok(found, `tokens.css has no --${name}`);
  return found[1].trim();
};

test('tokens.css states no value of a theme step: the pack decides them', () => {
  const stated = [...css.matchAll(/^\s*--(text-[\w]+?(?:--line-height)?|radius-[\w]+|spacing|transition-duration-\w+|ease-\w+):\s*([^;]+);/gm)]
    .filter(([, , value]) => !/var\(--/.test(value)).map(([, name]) => name);
  assert.deepEqual(stated, []);
});

// tokens.names.json is the one statement of Liuli's names; `lingxia gen tokens --names` writes an
// app's tokens from it, and tokens.css must say the same.
test('control heights are the names map\'s multiples of the spacing step', () => {
  for (const [size, units] of Object.entries(names.controlHeight)) assert.equal(token(`spacing-control-${size}`), `calc(var(--spacing) * ${units})`, `control-${size}`);
});

test('a control height and a control radius lower to steps of the pack, not to numbers', async () => {
  const slot = name => scales.findIndex(entry => entry.name === name);
  const rules = Object.fromEntries((await compileUtilities(new Set(['min-h-control-md', 'size-control-sm', 'rounded-control']), css)).map(rule => [rule.name, rule]));
  assert.deepEqual(rules['min-h-control-md'].style.minHeight, [slot('space'), 10, 0]);
  assert.deepEqual(rules['size-control-sm'].style.width, [slot('space'), 8, 0]);
  assert.deepEqual(rules['rounded-control'].style.borderRadius, [slot('radius.lg'), 1, 0]);
});

test('Liuli\'s own names are the names map\'s steps of those ramps, never lengths of their own', () => {
  for (const [name, step] of Object.entries(names.type)) {
    assert.ok(step in pack.type.scale, `text-${name}: the pack has no ${step}`);
    assert.equal(token(`text-${name}`), `var(--text-${step})`, `text-${name}`);
    assert.equal(token(`text-${name}--line-height`), `var(--text-${step}--line-height)`, `text-${name} line height`);
  }
  for (const [name, step] of Object.entries(names.radius)) {
    assert.ok(step in pack.shape.radius, `radius-${name}: the pack has no ${step}`);
    assert.equal(token(`radius-${name}`), `var(--radius-${step})`, `radius-${name}`);
  }
});

test('tokens.css names nothing the names map does not', () => {
  const ramp = new Set([...Object.keys(pack.type.scale), ...Object.keys(pack.shape.radius)]);
  const stated = [...css.matchAll(/^\s*--(text|radius)-([\w]+):/gm)].map(([, kind, name]) => [kind, name]).filter(([, name]) => !ramp.has(name));
  const known = { text: names.type, radius: names.radius };
  assert.deepEqual(stated.filter(([kind, name]) => !(name in known[kind])).map(([kind, name]) => `${kind}-${name}`), []);
});

test('every shadow layer is the theme\'s shadow role at an alpha', () => {
  for (const name of ['shadow-contact', 'shadow-raised', 'shadow-modal']) {
    const layers = token(name).split(/,(?![^(]*\))/);
    for (const layer of layers) assert.match(layer, /--alpha\(var\(--color-shadow\) \/ \d+%\)$/, `${name}: ${layer.trim()}`);
  }
});
