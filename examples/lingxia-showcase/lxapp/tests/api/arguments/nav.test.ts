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
  'surface.openPage': {
    refuseOnly: true,
    rows: [
      ['no page', [], 'E_INVALID_ARG'],
      ['number page', [1], 'E_INVALID_ARG'],
      ['undeclared page', ['nosuchpage'], 'E_NOT_FOUND'],
      ['options not object', ['home', 'x'], 'E_INVALID_ARG'],
      ['unknown as', ['home', { as: 'moon' }], 'E_INVALID_ARG'],
    ],
  },
  'surface.openUrl': {
    refuseOnly: true,
    rows: [
      ['no url', [], 'E_INVALID_ARG'],
      ['number url', [1], 'E_INVALID_ARG'],
      ['relative url', ['notaurl'], 'E_INVALID_ARG'],
      ['plain http', ['http://example.com'], 'E_INVALID_ARG'],
      ['options not object', ['https://example.com', 'x'], 'E_INVALID_ARG'],
      ['unknown option', ['https://example.com', { bogus: 1 }], 'E_INVALID_ARG'],
      ['edge without aside', ['https://example.com', { edge: 'left' }], 'E_INVALID_ARG'],
    ],
  },
  'surface.openApp': {
    refuseOnly: true,
    rows: [
      ['no app id', [], 'E_INVALID_ARG'],
      ['number app id', [1], 'E_INVALID_ARG'],
      ['empty app id', [''], 'E_INVALID_ARG'],
      ['options not object', ['x', 'y'], 'E_INVALID_ARG'],
      ['unknown as', ['x', { as: 'moon' }], 'E_INVALID_ARG'],
      ['unknown option', ['x', { bogus: 1 }], 'E_INVALID_ARG'],
      ['app not installed', ['no.such.app'], 'E_NOT_FOUND'],
    ],
  },
  'surface.openDeclared': {
    refuseOnly: true,
    rows: [
      ['no id', [], 'E_INVALID_ARG'],
      ['number id', [1], 'E_INVALID_ARG'],
      ['options not object', ['x', 'y'], 'E_INVALID_ARG'],
      ['undeclared id', ['nosuch'], 'E_NOT_FOUND'],
    ],
  },
  'surface.get': [
    ['no key', [], 'E_INVALID_ARG'],
    ['number key', [1], 'E_INVALID_ARG'],
    ['unknown key', ['nosuch'], 'ok'],
  ],
  'surface.app': [
    ['no app id', [], 'E_INVALID_ARG'],
    ['number app id', [1], 'E_INVALID_ARG'],
    ['unknown app', ['nosuch'], 'ok'],
  ],
  'surface.declared': [
    ['no id', [], 'E_INVALID_ARG'],
    ['number id', [1], 'E_INVALID_ARG'],
    ['options not object', ['nosuch', 'x'], 'E_INVALID_ARG'],
    ['undeclared id', ['nosuch'], 'ok'],
  ],
});
