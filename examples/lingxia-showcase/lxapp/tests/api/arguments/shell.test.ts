import { argumentContracts } from './contract.js';

// The shell chrome is shared with the rest of the suite, so these only refuse.
argumentContracts({
  'shell.appDisplay.update': {
    refuseOnly: true,
    rows: [
      ['no patch', [], 'E_INVALID_ARG'],
      ['patch not object', ['x'], 'E_INVALID_ARG'],
      ['empty patch', [{}], 'E_INVALID_ARG'],
      ['unknown field', [{ bogus: 1 }], 'E_INVALID_ARG'],
    ],
  },
  'shell.sidebarActions.replace': {
    refuseOnly: true,
    rows: [
      ['no items', [], 'E_INVALID_ARG'],
      ['items not array', ['x'], 'E_INVALID_ARG'],
      ['item without id', [[{}]], 'E_INVALID_ARG'],
      ['empty id', [[{ id: '' }]], 'E_INVALID_ARG'],
    ],
  },
  'shell.sidebarActions.update': {
    refuseOnly: true,
    rows: [
      ['no arguments', [], 'E_INVALID_ARG'],
      ['no patch', ['nosuch'], 'E_INVALID_ARG'],
      ['number id', [1, { label: 'x' }], 'E_INVALID_ARG'],
      ['patch not object', ['nosuch', 'x'], 'E_INVALID_ARG'],
      ['empty patch', ['nosuch', {}], 'E_INVALID_ARG'],
      ['unknown id', ['nosuch', { label: 'x' }], 'E_NOT_FOUND'],
    ],
  },
  'shell.sidebarActions.remove': {
    refuseOnly: true,
    rows: [
      ['no id', [], 'E_INVALID_ARG'],
      ['number id', [1], 'E_INVALID_ARG'],
      ['unknown id', ['nosuch'], 'E_NOT_FOUND'],
    ],
  },
});
