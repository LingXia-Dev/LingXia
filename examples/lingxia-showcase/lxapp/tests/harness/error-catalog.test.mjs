import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { families, runtimeOwners } from '../error-catalog.mjs';

const root = new URL('../../../../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const reasons = read('packages/lingxia-types/src/generated/logic.ts')
  .match(/export type LxErrorReason\s*=([^;]*);/)?.[1];
const bridge = read('packages/lingxia-bridge/src/types.ts')
  .match(/export const BRIDGE_ERROR\s*=\s*\{([\s\S]*?)\}/)?.[1];
const errorModule = read('packages/lingxia-types/src/error.ts');
const declarations = {
  LxErrorCode: [...(errorModule.match(/export const LX_ERROR_CODES = \[([\s\S]*?)\]/)?.[1] ?? '').matchAll(/'(E_[A-Z_]+)'/g)].map((match) => match[1]),
  LxErrorReason: [...(reasons ?? '').matchAll(/'([A-Za-z]+)'/g)].map((match) => match[1]),
  'BRIDGE_*': [...(bridge ?? '').matchAll(/'(BRIDGE_[A-Z_]+)'/g)].map((match) => match[1]),
};

test('every published error code has an executable owner and an explicit coverage layer', () => {
  assert.deepEqual(families.map((item) => item.family).sort(), Object.keys(declarations).sort());
  for (const family of families) {
    assert.ok(declarations[family.family].length > 0, `missing declarations for ${family.family}`);
    assert.equal(new Set(family.codes).size, family.codes.length, 'duplicate code');
    assert.deepEqual([...family.codes].sort(), declarations[family.family].sort(), `${family.family}: update ownership when codes change`);
    assert.ok(['normalization', 'error-envelope'].includes(family.layer));
    assert.ok(family.remaining.length > 0, 'handling coverage must not imply exhaustive native failure coverage');
    assert.match(read(family.owner), /assert(\.|_eq!|!)/, `owner must be an executable assertion file: ${family.owner}`);
  }
  const codes = new Set(families.flatMap((family) => family.codes));
  for (const owner of runtimeOwners) {
    assert.ok(codes.has(owner.code));
    const source = read(owner.file);
    assert.ok(source.includes(owner.id), `missing runtime owner ${owner.id}`);
    assert.ok(source.includes(String(owner.code)), `runtime owner does not mention ${owner.code}`);
  }
});
