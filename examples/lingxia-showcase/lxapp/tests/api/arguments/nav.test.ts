import { argumentContracts, subscriptionRows, type Row } from './contract.js';

// A valid move would take the suite's page away, so navigation only refuses.
const targetRows: readonly Row[] = [
  ['no target', [], 'E_INVALID_ARG'],
  ['target not object', ['x'], 'E_INVALID_ARG'],
  ['empty target', [{}], 'E_INVALID_ARG'],
  ['number page', [{ page: 1 }], 'E_INVALID_ARG'],
  ['undeclared page', [{ page: 'nosuchpage' }], 'E_NOT_FOUND'],
  ['undeclared tab', [{ tab: 'nosuchtab' }], 'E_NOT_FOUND'],
];

argumentContracts({
  'nav.go': {
    refuseOnly: true,
    rows: [
      ...targetRows,
      ['unknown key', [{ page: 'home', extra: 1 }], 'E_INVALID_ARG'],
      ['empty app', [{ app: '' }], 'E_INVALID_ARG'],
      ['relative url', [{ url: 'notaurl' }], 'E_INVALID_ARG'],
      ['number url', [{ url: 1 }], 'E_INVALID_ARG'],
    ],
  },
  'nav.reset': { refuseOnly: true, rows: targetRows },
  'nav.current.watch': subscriptionRows(),
  'nav.back': {
    refuseOnly: true,
    rows: [
      ['string count', ['x'], 'E_INVALID_ARG'],
      ['zero', [0], 'E_INVALID_ARG'],
      ['negative', [-1], 'E_INVALID_ARG'],
      ['fraction', [1.5], 'E_INVALID_ARG'],
    ],
  },
  'surface.open': {
    refuseOnly: true,
    rows: [
      ['no target', [], 'E_INVALID_ARG'],
      ['target not object', ['home'], 'E_INVALID_ARG'],
      ['empty target', [{}], 'E_INVALID_ARG'],
      ['tab target', [{ tab: 'home' }], 'E_INVALID_ARG'],
      ['two kinds', [{ url: 'https://example.com', app: 'x' }], 'E_INVALID_ARG'],
      ['number page', [{ page: 1 }], 'E_INVALID_ARG'],
      ['undeclared page', [{ page: 'nosuchpage' }], 'E_NOT_FOUND'],
      ['options not object', [{ page: 'home' }, 'x'], 'E_INVALID_ARG'],
      ['unknown as', [{ page: 'home' }, { as: 'moon' }], 'E_INVALID_ARG'],
      ['params in options', [{ page: 'home' }, { params: {} }], 'E_INVALID_ARG'],
      ['number url', [{ url: 1 }], 'E_INVALID_ARG'],
      ['relative url', [{ url: 'notaurl' }], 'E_INVALID_ARG'],
      ['plain http', [{ url: 'http://example.com' }], 'E_INVALID_ARG'],
      ['unknown url option', [{ url: 'https://example.com' }, { bogus: 1 }], 'E_INVALID_ARG'],
      ['edge without aside', [{ url: 'https://example.com' }, { edge: 'left' }], 'E_INVALID_ARG'],
      ['empty app id', [{ app: '' }], 'E_INVALID_ARG'],
      ['unknown as for an app', [{ app: 'x' }, { as: 'moon' }], 'E_INVALID_ARG'],
      ['app page in options', [{ app: 'x' }, { page: 'home' }], 'E_INVALID_ARG'],
      ['app not installed', [{ app: 'no.such.app' }], 'E_NOT_FOUND'],
      ['number declared id', [{ declared: 1 }], 'E_INVALID_ARG'],
      ['undeclared id', [{ declared: 'nosuch' }], 'E_NOT_FOUND'],
    ],
  },
  'surface.get': [
    ['no query', [], 'E_INVALID_ARG'],
    ['bare key', ['nosuch'], 'E_INVALID_ARG'],
    ['number key', [{ key: 1 }], 'E_INVALID_ARG'],
    ['unknown key', [{ key: 'nosuch' }], 'ok'],
    ['unknown app', [{ app: 'nosuch' }], 'ok'],
    ['undeclared id', [{ declared: 'nosuch' }], 'ok'],
    ['number declared key', [{ declared: 'nosuch', key: 1 }], 'E_INVALID_ARG'],
  ],
});
