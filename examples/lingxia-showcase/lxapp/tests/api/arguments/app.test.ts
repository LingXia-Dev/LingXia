import { FN, argumentContracts, subscriptionRows } from './contract.js';

argumentContracts({
  'app.t': [
    ['no key', [], 'E_INVALID_ARG'],
    ['number key', [1], 'E_INVALID_ARG'],
    ['unknown key', ['no.such.key'], 'E_NOT_FOUND'],
    ['values not object', ['app.update.title', 5], 'E_INVALID_ARG'],
    ['key', ['app.update.title'], 'ok'],
    ['unused values', ['app.update.title', { extra: 1 }], 'ok'],
  ],
  // Both replace chrome the rest of the suite reads, so only refusals run.
  'app.moreActions.replace': {
    refuseOnly: true,
    rows: [
      ['no items', [], 'E_INVALID_ARG'],
      ['items not array', ['x'], 'E_INVALID_ARG'],
      ['empty item', [[{}]], 'E_INVALID_ARG'],
      ['no onSelect', [[{ icon: 'a', label: 'b' }]], 'E_INVALID_ARG'],
      ['number label', [[{ icon: 'a', label: 1, onSelect: FN }]], 'E_INVALID_ARG'],
    ],
  },
  'app.tabBar.update': {
    refuseOnly: true,
    rows: [
      ['no patch', [], 'E_INVALID_ARG'],
      ['patch not object', ['x'], 'E_INVALID_ARG'],
      ['unknown visibility', [{ visibility: 'maybe' }], 'E_INVALID_ARG'],
      ['items not object', [{ items: 'x' }], 'E_INVALID_ARG'],
      ['unknown tab', [{ items: { nosuch: { badge: '1' } } }], 'E_INVALID_ARG'],
      ['unknown field', [{ color: 'red' }], 'E_INVALID_ARG'],
    ],
  },
  'app.appearance.watch': subscriptionRows(),
  'app.language.watch': subscriptionRows(),
  'app.update.onReady': subscriptionRows(),
  'app.update.onFailed': subscriptionRows(),
});
