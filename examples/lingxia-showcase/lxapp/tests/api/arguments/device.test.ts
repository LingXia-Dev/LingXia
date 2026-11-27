import { ABORTED, argumentContracts, subscriptionRows } from './contract.js';

argumentContracts({
  'device.haptics': {
    feature: 'device.haptics',
    rows: [
      ['number kind', [1], 'E_INVALID_ARG'],
      ['unknown kind', ['medium'], 'E_INVALID_ARG'],
    ],
  },
  // A valid call asks for location access, so only refusals run.
  'device.location': {
    refuseOnly: true,
    rows: [
      ['options not object', ['x'], 'E_INVALID_ARG'],
      ['unknown coordinates', [{ coordinates: 'mars' }], 'E_INVALID_ARG'],
      ['signal not a signal', [{ signal: 1 }], 'E_INVALID_ARG'],
      ['aborted signal', [{ signal: ABORTED }], 'E_ABORT'],
    ],
  },
  'device.onKeyDown': subscriptionRows(),
  'device.onKeyUp': subscriptionRows(),
  'device.orientation.watch': subscriptionRows(),
  'device.permission.get': [
    ['no name', [], 'E_INVALID_ARG'],
    ['number name', [1], 'E_INVALID_ARG'],
    ['unknown name', ['microphone'], 'E_INVALID_ARG'],
  ],
  // A known name may prompt, so only refusals run.
  'device.permission.request': {
    refuseOnly: true,
    rows: [
      ['no name', [], 'E_INVALID_ARG'],
      ['number name', [1], 'E_INVALID_ARG'],
      ['unknown name', ['microphone'], 'E_INVALID_ARG'],
    ],
  },
  'network.status.watch': subscriptionRows(),
  supports: [
    ['no feature', [], 'E_INVALID_ARG'],
    ['number feature', [1], 'E_INVALID_ARG'],
    ['unknown feature', ['no.such'], 'ok'],
    ['known feature', ['host.tray'], 'ok'],
  ],
});
