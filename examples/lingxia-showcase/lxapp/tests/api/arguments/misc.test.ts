import { argumentContracts } from './contract.js';

// Each of these hands off to another app when it succeeds, so they only refuse.
argumentContracts({
  openFile: {
    refuseOnly: true,
    rows: [
      ['no path', [], 'E_INVALID_ARG'],
      ['number path', [1], 'E_INVALID_ARG'],
      ['empty path', [''], 'E_INVALID_ARG'],
      ['native absolute', ['/etc/hosts'], 'E_INVALID_ARG'],
      ['options not object', ['lx://userdata/missing.pdf', 'x'], 'E_INVALID_ARG'],
      ['missing file', ['lx://userdata/missing.pdf'], 'E_NOT_FOUND'],
    ],
  },
  share: {
    refuseOnly: true,
    rows: [
      ['no options', [], 'E_INVALID_ARG'],
      ['options not object', ['x'], 'E_INVALID_ARG'],
      ['nothing to share', [{}], 'E_INVALID_ARG'],
      ['number text', [{ text: 1 }], 'E_INVALID_ARG'],
      ['native absolute file', [{ files: ['/etc/hosts'] }], 'E_INVALID_ARG'],
    ],
  },
  openExternal: {
    refuseOnly: true,
    rows: [
      ['no url', [], 'E_INVALID_ARG'],
      ['number url', [1], 'E_INVALID_ARG'],
      ['relative url', ['notaurl'], 'E_INVALID_ARG'],
      ['script scheme', ['javascript:alert(1)'], 'E_PERMISSION_DENIED'],
    ],
  },
});
