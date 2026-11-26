import assert from 'node:assert/strict';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { probe } from '@liulidesign/core/internal/probe';

// Every example drawn with the theme's steps left as references: a pack moves all of it.
const found = await probe(fileURLToPath(new URL('..', import.meta.url)));

test('every length drawn is a theme step, so a pack reaches it', () => {
  assert.deepEqual(found.lengths, []);
});

test('every colour drawn is a theme role', () => {
  assert.deepEqual(found.colours, []);
});

test('in high contrast every edge stands 3:1 off what it lies on', () => {
  assert.deepEqual(found.edges.map(({ example, edge, ratio }) => `${example}: ${edge}: ${ratio}`), []);
});
