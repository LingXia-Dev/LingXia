import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

const reactUi = readFileSync(new URL('../../pages/ui/index.tsx', import.meta.url), 'utf8');

test('React exposes the redirected UI instance identity', () => {
  assert.match(reactUi, /data-testid="ui-page"/);
  assert.match(reactUi, /data-instance-tag=\{instanceTag\}/);
});
