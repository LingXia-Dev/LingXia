import { FN, argumentContracts } from './contract.js';

const key = 'args.{ns}';

argumentContracts({
  'storage.get': [
    ['no key', [], 'E_INVALID_ARG'],
    ['number key', [1], 'E_INVALID_ARG'],
    ['options not object', [key, 'x'], 'E_INVALID_ARG'],
    ['decode not function', [key, { decode: 1 }], 'E_INVALID_ARG'],
    ['absent key', [key], 'ok'],
    ['decode option', [key, { decode: FN }], 'ok'],
  ],
  'storage.set': {
    rows: [
      ['no entry', [], 'E_INVALID_ARG'],
      ['key only', [key], 'E_INVALID_ARG'],
      ['number key', [1, 'v'], 'E_INVALID_ARG'],
      ['function value', [key, FN], 'E_INVALID_ARG'],
      ['key over 1 KiB', ['k'.repeat(1025), 'v'], 'E_OUT_OF_RANGE'],
      ['value over 5 MiB', [key, 'x'.repeat(5 * 1024 * 1024 + 1)], 'E_OUT_OF_RANGE'],
      ['object value', [key, { a: 1 }], 'ok'],
      ['null value', [key, null], 'ok'],
    ],
    cleanup: [['storage.delete', [key]]],
  },
  'storage.has': [
    ['no key', [], 'E_INVALID_ARG'],
    ['number key', [1], 'E_INVALID_ARG'],
    ['absent key', [key], 'ok'],
  ],
  'storage.delete': [
    ['no key', [], 'E_INVALID_ARG'],
    ['number key', [1], 'E_INVALID_ARG'],
    ['absent key resolves', [key], 'ok'],
  ],
  'storage.list': [
    ['number prefix', [1], 'E_INVALID_ARG'],
    ['no prefix', [], 'ok'],
    ['prefix', ['args.'], 'ok'],
  ],
});
