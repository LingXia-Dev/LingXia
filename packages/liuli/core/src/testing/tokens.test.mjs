import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

// Liuli states the default theme pack's ramps so its names resolve without an app's generated
// tokens. They are one source only while they are the pack's own values, which this proves.
const css = readFileSync(new URL('../../tokens.css', import.meta.url), 'utf8');
const names = JSON.parse(readFileSync(new URL('../../tokens.names.json', import.meta.url), 'utf8'));
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

// tokens.names.json is the one statement of Liuli's names; `lingxia gen tokens --names` writes an
// app's tokens from it, and tokens.css must say the same.
test('control heights are the names map\'s units of the pack\'s spacing unit', () => {
  for (const [size, units] of Object.entries(names.controlHeight)) assert.equal(points(token(`spacing-control-${size}`)), units * pack.space.unit, `control-${size}`);
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
