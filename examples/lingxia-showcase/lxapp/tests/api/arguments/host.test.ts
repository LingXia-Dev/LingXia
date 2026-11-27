import { ABORTED, FN, argumentContracts, subscriptionRows, type Row } from './contract.js';

// Valid calls badge the dock, show banners, notify or switch the host's
// preferences for everyone else on the session, so most tables only refuse.
const preferenceSetRows = (bad: string): readonly Row[] => [
  ['no value', [], 'E_INVALID_ARG'],
  ['number value', [1], 'E_INVALID_ARG'],
  ['unknown value', [bad], 'E_INVALID_ARG'],
];

const idRows = (): readonly Row[] => [
  ['no id', [], 'E_INVALID_ARG'],
  ['number id', [1], 'E_INVALID_ARG'],
  ['empty id', [''], 'E_INVALID_ARG'],
  ['unknown id resolves', ['nosuch'], 'ok'],
];

argumentContracts({
  'host.badge.set': {
    feature: 'host.badge',
    refuseOnly: true,
    rows: [
      ['no value', [], 'E_INVALID_ARG'],
      ['empty text', [''], 'E_INVALID_ARG'],
      ['negative count', [-1], 'E_INVALID_ARG'],
      ['fractional count', [1.5], 'E_INVALID_ARG'],
      ['object value', [{}], 'E_INVALID_ARG'],
      ['options not object', [1, 'x'], 'E_INVALID_ARG'],
      ['unknown surface', [1, { surface: 'moon' }], 'E_INVALID_ARG'],
    ],
  },
  'host.badge.clear': {
    feature: 'host.badge',
    refuseOnly: true,
    rows: [
      ['options not object', ['x'], 'E_INVALID_ARG'],
      ['unknown surface', [{ surface: 'moon' }], 'E_INVALID_ARG'],
    ],
  },
  'host.banner.show': {
    feature: 'host.banner',
    refuseOnly: true,
    rows: [
      ['no options', [], 'E_INVALID_ARG'],
      ['options not object', ['x'], 'E_INVALID_ARG'],
      ['no title', [{}], 'E_INVALID_ARG'],
      ['number title', [{ title: 1 }], 'E_INVALID_ARG'],
      ['three actions', [{ title: 't', actions: [{}, {}, {}] }], 'E_INVALID_ARG'],
      ['unknown background', [{ title: 't', background: 'neon' }], 'E_INVALID_ARG'],
    ],
  },
  'host.banner.dismiss': { feature: 'host.banner', rows: idRows() },
  'host.notification.show': {
    feature: 'host.notification',
    refuseOnly: true,
    rows: [
      ['no options', [], 'E_INVALID_ARG'],
      ['options not object', ['x'], 'E_INVALID_ARG'],
      ['no title', [{}], 'E_INVALID_ARG'],
      ['number title', [{ title: 1 }], 'E_INVALID_ARG'],
    ],
  },
  'host.notification.cancel': { feature: 'host.notification', rows: idRows() },
  'host.screenshot': [
    ['options not object', ['x'], 'E_INVALID_ARG'],
    ['window id not a window', [{ windowId: 'x' }], 'E_INVALID_ARG'],
    ['current window', [], 'ok'],
  ],
  'host.serviceEnv.set': {
    refuseOnly: true,
    rows: preferenceSetRows('moon'),
  },
  'host.appearancePreference.set': { refuseOnly: true, rows: preferenceSetRows('purple') },
  'host.contrastPreference.set': { refuseOnly: true, rows: preferenceSetRows('max') },
  'host.languagePreference.set': { refuseOnly: true, rows: preferenceSetRows('not a tag!') },
  'host.appearancePreference.watch': subscriptionRows(),
  'host.contrastPreference.watch': subscriptionRows(),
  'host.languagePreference.watch': subscriptionRows(),
  'host.update.check': {
    refuseOnly: true,
    rows: [
      ['options not object', ['x'], 'E_INVALID_ARG'],
      ['signal not a signal', [{ signal: 1 }], 'E_INVALID_ARG'],
      ['aborted signal', [{ signal: ABORTED }], 'E_ABORT'],
    ],
  },
  'host.tray.update': {
    feature: 'host.tray',
    refuseOnly: true,
    rows: [
      ['no patch', [], 'E_INVALID_ARG'],
      ['patch not object', ['x'], 'E_INVALID_ARG'],
      ['number title', [{ title: 1 }], 'E_INVALID_ARG'],
      ['string visible', [{ visible: 'x' }], 'E_INVALID_ARG'],
      ['number icon', [{ icon: 1 }], 'E_INVALID_ARG'],
      ['missing icon', [{ icon: 'no/such.png' }], 'E_NOT_FOUND', {
        bug: 'the macOS tray ignores an icon it cannot load and resolves',
      }],
    ],
  },
  'host.tray.onClick': { feature: 'host.tray', rows: subscriptionRows() },
  'host.tray.menu.replace': {
    feature: 'host.tray',
    refuseOnly: true,
    rows: [
      ['no items', [], 'E_INVALID_ARG'],
      ['items not array', ['x'], 'E_INVALID_ARG'],
      ['empty item', [[{}]], 'E_INVALID_ARG'],
      ['number label', [[{ label: 1 }]], 'E_INVALID_ARG'],
      ['empty label', [[{ label: ' ' }]], 'E_INVALID_ARG'],
      ['onSelect not function', [[{ label: 'a', onSelect: 1 }]], 'E_INVALID_ARG'],
      ['string separator', [[{ separator: 'yes' }]], 'E_INVALID_ARG'],
    ],
  },
  // Terminal settings answer only the host's own control surface; elsewhere the gate refuses first.
  'host.terminal.settings.update': {
    feature: 'host.terminal',
    gatedFirst: true,
    refuseOnly: true,
    rows: [['patch not object', [1], 'E_INVALID_ARG']],
  },
  'host.terminal.settings.reset': {
    feature: 'host.terminal',
    gatedFirst: true,
    refuseOnly: true,
    rows: [['options not object', [1], 'E_INVALID_ARG']],
  },
  'host.terminal.settings.onChange': {
    feature: 'host.terminal',
    gatedFirst: true,
    rows: [['number listener', [1], 'E_INVALID_ARG'], ['listener', [FN], 'ok']],
  },
  'host.terminal.colorSchemes.import': {
    feature: 'host.terminal',
    gatedFirst: true,
    refuseOnly: true,
    rows: [['options not object', [1], 'E_INVALID_ARG'], ['no text', [{}], 'E_INVALID_ARG']],
  },
  'host.terminal.colorSchemes.importFile': {
    feature: 'host.terminal',
    gatedFirst: true,
    refuseOnly: true,
    rows: [['number path', [1], 'E_INVALID_ARG']],
  },
});
