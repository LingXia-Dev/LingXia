import { ABORTED, argumentContracts, type Row } from './contract.js';

// A dialog that is shown waits for a person, so these tables only refuse.
const dialogRows: readonly Row[] = [
  ['no options', [], 'E_INVALID_ARG'],
  ['number options', [1], 'E_INVALID_ARG'],
  ['no title', [{}], 'E_INVALID_ARG'],
  ['empty title', [{ title: '' }], 'E_INVALID_ARG'],
  ['empty string', [''], 'E_INVALID_ARG'],
  ['number title', [{ title: 1 }], 'E_INVALID_ARG'],
  ['unknown tone', [{ title: 't', tone: 'loud' }], 'E_INVALID_ARG'],
  ['signal not a signal', [{ title: 't', signal: 1 }], 'E_INVALID_ARG'],
  ['aborted signal', [{ title: 't', signal: ABORTED }], 'E_ABORT'],
];

argumentContracts({
  'ui.alert': { refuseOnly: true, rows: dialogRows },
  'ui.confirm': { refuseOnly: true, rows: dialogRows },
  'ui.actionSheet': {
    refuseOnly: true,
    rows: [
      ['no options', [], 'E_INVALID_ARG'],
      ['options not object', ['x'], 'E_INVALID_ARG'],
      ['no items', [{}], 'E_INVALID_ARG'],
      ['empty items', [{ items: [] }], 'E_INVALID_ARG'],
      ['item without label', [{ items: [{ id: 'a' }] }], 'E_INVALID_ARG'],
      ['empty id', [{ items: [{ id: '', label: 'A' }] }], 'E_INVALID_ARG'],
      ['repeated id', [{ items: [{ id: 'a', label: 'A' }, { id: 'a', label: 'B' }] }], 'E_INVALID_ARG'],
      ['unknown tone', [{ items: [{ id: 'a', label: 'A' }], tone: 'loud' }], 'E_INVALID_ARG'],
      ['aborted signal', [{ items: [{ id: 'a', label: 'A' }], signal: ABORTED }], 'E_ABORT'],
    ],
  },
  'ui.toast': [
    ['no message', [], 'E_INVALID_ARG'],
    ['number message', [1], 'E_INVALID_ARG'],
    ['options not object', ['hi', 'x'], 'E_INVALID_ARG'],
    ['unknown tone', ['hi', { tone: 'loud' }], 'E_INVALID_ARG'],
    ['message', ['hi'], 'ok'],
  ],
});
