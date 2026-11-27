import { ABORTED, argumentContracts, type Row } from './contract.js';

// A valid call opens a system picker that waits for a person, so these only refuse.
const fileRows: readonly Row[] = [
  ['options not object', ['x'], 'E_INVALID_ARG'],
  ['filters not array', [{ filters: 'x' }], 'E_INVALID_ARG'],
  ['filter without extensions', [{ filters: [{}] }], 'E_INVALID_ARG'],
  ['number extensions', [{ filters: [{ extensions: 1 }] }], 'E_INVALID_ARG'],
  ['native default path', [{ defaultPath: '/etc' }], 'E_INVALID_ARG'],
  ['aborted signal', [{ signal: ABORTED }], 'E_ABORT'],
];

argumentContracts({
  'pick.file': { refuseOnly: true, rows: fileRows },
  'pick.files': { refuseOnly: true, rows: fileRows },
  'pick.directory': {
    feature: 'pick.directory',
    refuseOnly: true,
    rows: [
      ['options not object', ['x'], 'E_INVALID_ARG'],
      ['aborted signal', [{ signal: ABORTED }], 'E_ABORT'],
    ],
  },
  'pick.media': {
    refuseOnly: true,
    rows: [
      ['options not object', ['x'], 'E_INVALID_ARG'],
      ['empty sources', [{ sources: [] }], 'E_INVALID_ARG'],
      ['unknown source', [{ sources: ['moon'] }], 'E_INVALID_ARG'],
      ['unknown type', [{ types: ['moon'] }], 'E_INVALID_ARG'],
      ['string limit', [{ limit: 'x' }], 'E_INVALID_ARG'],
      ['zero limit', [{ limit: 0 }], 'E_INVALID_ARG'],
      ['aborted signal', [{ signal: ABORTED }], 'E_ABORT'],
    ],
  },
  'pick.capture': {
    feature: 'pick.camera',
    refuseOnly: true,
    rows: [
      ['no options', [], 'E_INVALID_ARG'],
      ['options not object', ['x'], 'E_INVALID_ARG'],
      ['no type', [{}], 'E_INVALID_ARG'],
      ['unknown type', [{ type: 'audio' }], 'E_INVALID_ARG'],
      ['aborted signal', [{ type: 'image', signal: ABORTED }], 'E_ABORT'],
    ],
  },
  'pick.code': {
    feature: 'pick.code',
    refuseOnly: true,
    rows: [
      ['options not object', ['x'], 'E_INVALID_ARG'],
      ['unknown format', [{ formats: ['moon'] }], 'E_INVALID_ARG'],
      ['aborted signal', [{ signal: ABORTED }], 'E_ABORT'],
    ],
  },
});
