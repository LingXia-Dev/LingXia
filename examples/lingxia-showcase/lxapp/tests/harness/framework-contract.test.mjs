import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

const ui = readFileSync(new URL('../../pages/ui/index.tsx', import.meta.url), 'utf8');

test('the ui page exposes the redirected instance identity', () => {
  assert.match(ui, /testId="ui-page"/);
  assert.match(ui, /'data-instance-tag': String\(data\.instanceTag/);
});
