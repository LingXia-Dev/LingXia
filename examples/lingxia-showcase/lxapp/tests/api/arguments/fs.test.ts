import { argumentContracts } from './contract.js';

const dir = 'lx://userdata/{ns}';
const file = `${dir}/a.txt`;
const other = `${dir}/b.txt`;
const missing = `${dir}/missing.txt`;
const reset = [['fs.remove', [dir, { recursive: true }]]] as const;
const seed = ['seed', [file, { data: 'a' }], 'ok', { via: 'fs.write' }] as const;
const seedOther = ['seed other', [other, { data: 'b' }], 'ok', { via: 'fs.write' }] as const;

argumentContracts({
  'fs.exists': [
    ['no path', [], 'E_INVALID_ARG'],
    ['number path', [1], 'E_INVALID_ARG'],
    ['missing file', [missing], 'ok'],
  ],
  'fs.file': [
    ['number path', [1], 'E_INVALID_ARG'],
    ['empty path', [''], 'E_INVALID_ARG'],
    ['native absolute', ['/etc/hosts'], 'E_INVALID_ARG'],
    ['managed path', [file], 'ok'],
  ],
  'fs.stat': [
    ['no path', [], 'E_INVALID_ARG'],
    ['number path', [1], 'E_INVALID_ARG'],
    ['empty path', [''], 'E_INVALID_ARG'],
    ['native absolute', ['/etc/hosts'], 'E_INVALID_ARG'],
    ['missing file', [missing], 'E_NOT_FOUND'],
  ],
  'fs.readDir': {
    rows: [
      ['no path', [], 'E_INVALID_ARG'],
      ['number path', [1], 'E_INVALID_ARG'],
      ['native absolute', ['/etc'], 'E_INVALID_ARG'],
      ['missing dir', [`${dir}/nowhere`], 'E_NOT_FOUND'],
      seed,
      ['existing dir', [dir], 'ok'],
    ],
    cleanup: reset,
  },
  'fs.mkdir': {
    rows: [
      ['number path', [1], 'E_INVALID_ARG'],
      ['options not object', [`${dir}/d`, 'x'], 'E_INVALID_ARG'],
      ['recursive not boolean', [`${dir}/d`, { recursive: 'yes' }], 'E_INVALID_ARG'],
      ['native absolute', ['/tmp/lingxia-args'], 'E_INVALID_ARG'],
      ['missing parent without recursive', [`${dir}/x/y`], 'E_NOT_FOUND'],
      ['recursive', [`${dir}/d/e`, { recursive: true }], 'ok'],
      ['existing dir resolves', [`${dir}/d`], 'ok'],
    ],
    cleanup: reset,
  },
  'fs.write': {
    rows: [
      ['no options', [file], 'E_INVALID_ARG'],
      ['no data', [file, {}], 'E_INVALID_ARG'],
      ['number data', [file, { data: 1 }], 'E_INVALID_ARG'],
      ['unknown encoding', [file, { data: 'x', encoding: 'hex' }], 'E_INVALID_ARG'],
      ['bad base64', [file, { data: '%%%', encoding: 'base64' }], 'E_INVALID_ARG'],
      ['overwrite not boolean', [file, { data: 'x', overwrite: 'no' }], 'E_INVALID_ARG'],
      ['native absolute', ['/tmp/lingxia-args.txt', { data: 'x' }], 'E_INVALID_ARG'],
      ['utf8 text', [file, { data: 'x' }], 'ok'],
      ['base64 over existing', [file, { data: 'eA==', encoding: 'base64' }], 'ok'],
      ['existing with overwrite false', [file, { data: 'x', overwrite: false }], 'E_ALREADY_EXISTS'],
    ],
    cleanup: reset,
  },
  'fs.copy': {
    rows: [
      ['no options', [file], 'E_INVALID_ARG'],
      ['no to', [file, {}], 'E_INVALID_ARG'],
      ['number to', [file, { to: 1 }], 'E_INVALID_ARG'],
      ['missing source', [missing, { to: other }], 'E_NOT_FOUND'],
      seed,
      ['to native absolute', [file, { to: '/tmp/lingxia-args.txt' }], 'E_INVALID_ARG'],
      ['copy', [file, { to: other }], 'ok'],
      ['existing destination', [file, { to: other }], 'E_ALREADY_EXISTS'],
      ['overwrite', [file, { to: other, overwrite: true }], 'ok'],
    ],
    cleanup: reset,
  },
  'fs.rename': {
    rows: [
      ['no options', [file], 'E_INVALID_ARG'],
      ['no to', [file, {}], 'E_INVALID_ARG'],
      ['number to', [file, { to: 1 }], 'E_INVALID_ARG'],
      ['to native absolute', [file, { to: '/tmp/lingxia-args.txt' }], 'E_INVALID_ARG'],
      ['overwrite not boolean', [file, { to: other, overwrite: 'y' }], 'E_INVALID_ARG'],
      ['missing source', [missing, { to: other }], 'E_NOT_FOUND'],
      seed,
      seedOther,
      ['existing destination', [file, { to: other }], 'E_ALREADY_EXISTS'],
      ['overwrite a file', [file, { to: other, overwrite: true }], 'ok'],
    ],
    cleanup: reset,
  },
  'fs.remove': {
    rows: [
      ['no path', [], 'E_INVALID_ARG'],
      ['number path', [1], 'E_INVALID_ARG'],
      ['recursive not boolean', [dir, { recursive: 1 }], 'E_INVALID_ARG'],
      ['storage root', ['lx://userdata/'], 'E_INVALID_ARG'],
      ['missing file', [missing], 'E_NOT_FOUND'],
      seed,
      ['non-empty dir without recursive', [dir], 'E_INVALID_STATE'],
      ['file', [file], 'ok'],
      ['directory recursive', [dir, { recursive: true }], 'ok'],
    ],
    cleanup: reset,
  },
});
