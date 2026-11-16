import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

// Liuli states the default theme pack's ramps so its names resolve without an app's generated
// tokens. They are one source only while they are the pack's own values, which this proves.
const css = readFileSync(new URL('../../tokens.css', import.meta.url), 'utf8');
const pack = JSON.parse(readFileSync(new URL('../../../../../crates/lingxia-theme/packs/default.json', import.meta.url), 'utf8'));
const token = name => {
  const found = css.match(new RegExp(`^\\s*--${name}:\\s*([^;]+);`, 'm'));
  assert.ok(found, `tokens.css has no --${name}`);
  return found[1].trim();
};
const points = value => {
  const rem = value.match(/^([\d.]+)rem$/);
  const px = value.match(/^([\d.]+)px$/);
  assert.ok(rem || px, `${value} is not a length`);
  return rem ? Number(rem[1]) * 16 : Number(px[1]);
};

test('the type ramp is the default pack\'s, size and line height', () => {
  for (const [step, size] of Object.entries(pack.type.scale)) {
    assert.equal(points(token(`text-${step}`)), size, `text-${step}`);
    assert.equal(points(token(`text-${step}--line-height`)), pack.type.leading[step], `text-${step} line height`);
  }
});

test('the radius ramp is the default pack\'s', () => {
  for (const [step, radius] of Object.entries(pack.shape.radius)) {
    if (step === 'DEFAULT' || step === 'full') continue;
    assert.equal(points(token(`radius-${step}`)), radius, `radius-${step}`);
  }
});

test('motion is the default pack\'s', () => {
  for (const step of ['fast', 'base', 'slow']) assert.equal(token(`transition-duration-${step}`), `${pack.motion[step]}ms`, `duration-${step}`);
  for (const [name, key] of [['standard', 'easing'], ['enter', 'enter'], ['exit', 'exit']]) assert.equal(token(`ease-${name}`), pack.motion[key], `ease-${name}`);
});

test('control heights are 8, 10 and 12 of the pack\'s spacing unit', () => {
  for (const [size, units] of [['sm', 8], ['md', 10], ['lg', 12]]) assert.equal(points(token(`spacing-control-${size}`)), units * pack.space.unit, `control-${size}`);
});

test('Liuli\'s own names are steps of those ramps, never lengths of their own', () => {
  const aliases = {
    'text-caption': 'text-xs', 'text-footnote': 'text-sm', 'text-body': 'text-base', 'text-callout': 'text-lg', 'text-title': 'text-xl', 'text-display': 'text-3xl',
    'radius-control': 'radius-lg', 'radius-container': 'radius-xl', 'radius-dialog': 'radius-2xl', 'radius-checkbox': 'radius-sm', 'radius-segment': 'radius-md',
  };
  for (const [name, step] of Object.entries(aliases)) {
    assert.equal(token(name), `var(--${step})`, name);
    if (name.startsWith('text-')) assert.equal(token(`${name}--line-height`), `var(--${step}--line-height)`, `${name} line height`);
  }
});

test('every shadow layer is the theme\'s shadow role at an alpha', () => {
  for (const name of ['shadow-contact', 'shadow-raised', 'shadow-modal']) {
    const layers = token(name).split(/,(?![^(]*\))/);
    for (const layer of layers) assert.match(layer, /--alpha\(var\(--color-shadow\) \/ \d+%\)$/, `${name}: ${layer.trim()}`);
  }
});

test('charts declares the same type scale as core, line for line', () => {
  const charts = readFileSync(new URL('../../../charts/tokens.css', import.meta.url), 'utf8');
  const declared = [...charts.matchAll(/^\s*--([\w-]+):\s*([^;]+);/gm)];
  assert.ok(declared.length > 20, 'the ramp and its aliases');
  for (const [, name, value] of declared) assert.equal(value.trim(), token(name), `charts --${name}`);
});
