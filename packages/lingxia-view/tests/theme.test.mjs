import { test } from 'node:test';
import assert from 'node:assert/strict';
import { describe, h } from '../src/internal.js';
import { roleUnder } from '../src/theme.js';

test('lines inside a lifted surface take its pair; other roles and surfaces keep theirs', () => {
  assert.equal(roleUnder('line', 'surface-raised'), 'line-on-raised');
  assert.equal(roleUnder('line-strong', 'surface-overlay'), 'line-strong-on-overlay');
  assert.equal(roleUnder('line', 'surface'), 'line');
  assert.equal(roleUnder('accent', 'surface-overlay'), 'accent');
});

test('a page cannot name an internal role', () => {
  for (const role of ['line-on-raised', 'line-strong-on-overlay/50'])
    assert.throws(() => describe(() => h('view', { style: { borderColor: role, borderWidth: 1 } }), {}), /UI_COLOR/);
  describe(() => h('view', { style: { borderColor: 'line', borderWidth: 1 } }), {});
});
