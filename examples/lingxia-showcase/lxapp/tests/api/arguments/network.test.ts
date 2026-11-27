import { ABORTED, argumentContracts } from './contract.js';

const url = 'https://example.com/a';
const missing = 'lx://userdata/{ns}/missing.bin';

// A valid transfer reaches the network, so these only refuse.
argumentContracts({
  'network.download': {
    refuseOnly: true,
    rows: [
      ['no url', [], 'E_INVALID_ARG'],
      ['number url', [1], 'E_INVALID_ARG'],
      ['relative url', ['notaurl'], 'E_INVALID_ARG'],
      ['ftp url', ['ftp://example.com/a'], 'E_INVALID_ARG'],
      ['options not object', [url, 'x'], 'E_INVALID_ARG'],
      ['number to', [url, { to: 1 }], 'E_INVALID_ARG', { then: 'result' }],
      ['headers not object', [url, { headers: 1 }], 'E_INVALID_ARG'],
      ['signal not a signal', [url, { signal: 1 }], 'E_INVALID_ARG'],
      ['aborted signal', [url, { signal: ABORTED }], 'E_ABORT', { then: 'result' }],
    ],
  },
  'network.upload': {
    refuseOnly: true,
    rows: [
      ['no arguments', [], 'E_INVALID_ARG'],
      ['no options', [url], 'E_INVALID_ARG'],
      ['number url', [1, { path: missing }], 'E_INVALID_ARG'],
      ['relative url', ['notaurl', { path: missing }], 'E_INVALID_ARG'],
      ['options not object', [url, 'x'], 'E_INVALID_ARG'],
      ['no path', [url, {}], 'E_INVALID_ARG'],
      ['native absolute path', [url, { path: '/etc/hosts' }], 'E_INVALID_ARG'],
      ['missing file', [url, { path: missing }], 'E_NOT_FOUND'],
    ],
  },
  'network.wifi.connect': {
    feature: 'network.wifi',
    refuseOnly: true,
    rows: [
      ['no options', [], 'E_INVALID_ARG'],
      ['options not object', ['x'], 'E_INVALID_ARG'],
      ['no ssid', [{}], 'E_INVALID_ARG'],
      ['number ssid', [{ ssid: 1 }], 'E_INVALID_ARG'],
    ],
  },
  'network.wifi.list': {
    feature: 'network.wifi',
    refuseOnly: true,
    rows: [
      ['options not object', ['x'], 'E_INVALID_ARG'],
      ['aborted signal', [{ signal: ABORTED }], 'E_ABORT'],
    ],
  },
  'network.wifi.onConnected': {
    feature: 'network.wifi',
    // Subscribing on a host without Wi-Fi is inert rather than refused.
    rows: [['no callback', [], 'E_INVALID_ARG'], ['number callback', [1], 'E_INVALID_ARG']],
  },
});
