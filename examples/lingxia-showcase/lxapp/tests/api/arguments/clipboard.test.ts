import { argumentContracts } from './contract.js';

// Valid writes would replace the developer's real clipboard, so only reads succeed here.
argumentContracts({
  'clipboard.read': [
    ['options not object', ['text'], 'E_INVALID_ARG'],
    ['unknown type', [{ type: 'html' }], 'E_INVALID_ARG'],
    ['number type', [{ type: 1 }], 'E_INVALID_ARG'],
    ['text only', [{ type: 'text' }], 'ok'],
    ['every type', [], 'ok'],
  ],
  'clipboard.write': [
    ['no item', [], 'E_INVALID_ARG'],
    ['item not object', ['text'], 'E_INVALID_ARG'],
    ['unknown type', [{ type: 'html', text: 'x' }], 'E_INVALID_ARG'],
    ['text missing', [{ type: 'text' }], 'E_INVALID_ARG'],
    ['image empty path', [{ type: 'image', path: '' }], 'E_INVALID_ARG'],
    ['image native absolute', [{ type: 'image', path: '/tmp/a.png' }], 'E_INVALID_ARG'],
  ],
  'clipboard.writeText': [
    ['no text', [], 'E_INVALID_ARG'],
    ['number text', [1], 'E_INVALID_ARG'],
    ['over 1 MiB', ['x'.repeat(1024 * 1024 + 1)], 'E_INVALID_ARG'],
  ],
});
