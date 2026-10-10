// Owners are repository-relative executable tests. Normalization/envelope
// coverage does not imply that every native business failure was induced.
export const families = [
  {
    family: 'LxErrorCode', layer: 'normalization',
    owner: 'packages/lingxia-types/tests/error-runtime.mjs',
    codes: [1000, 1001, 1002, 1003, 1004, 1005, 2000, 2001,
      3000, 3001, 3002, 3003, 3004, 3005, 3006, 3007, 3008,
      4000, 4001, 4002, 4003, 4004, 5000, 5001, 5002, 5003, 5004,
      6000, 6001, 6002, 12000, 12001, 12002, 12003, 12004, 12005,
      12006, 12007, 12008, 12009, 12010],
    remaining: 'Native production of every business failure needs individual API, permission, network, and host-version fixtures.',
  },
  {
    family: 'SurfaceErrorCode', layer: 'normalization',
    owner: 'packages/lingxia-types/tests/error-runtime.mjs',
    codes: ['unsupported_placement', 'denied', 'not_declared', 'invalid_arg',
      'already_open_other_role', 'closed', 'capability_missing', 'failed'],
    remaining: 'Placement, role conflicts, and host failures need the corresponding desktop/mobile presenter and fault fixture.',
  },
  {
    family: 'BRIDGE_*', layer: 'error-envelope',
    owner: 'packages/lingxia-bridge/test-support/test-channel-errors.mjs',
    codes: ['BRIDGE_NOT_READY', 'BRIDGE_TIMEOUT', 'BRIDGE_CANCELED',
      'BRIDGE_PROTOCOL_MISMATCH', 'BRIDGE_HANDSHAKE_FAILED', 'BRIDGE_MALFORMED_MESSAGE',
      'BRIDGE_METHOD_NOT_FOUND', 'BRIDGE_TOPIC_NOT_FOUND', 'BRIDGE_CAPABILITY_DENIED',
      'BRIDGE_INTERNAL_ERROR', 'BRIDGE_OUTBOX_FULL', 'BRIDGE_STREAM_OVERFLOW',
      'BRIDGE_STREAM_CLOSED', 'BRIDGE_MESSAGE_TOO_LARGE'],
    remaining: 'Error-envelope tests inject rejected acknowledgements; overflow, cancellation, and handshake failure origins are not exhaustively induced here.',
  },
];

// Stronger real-device contracts are listed separately from code handling.
export const runtimeOwners = [
  { code: 4004, id: 'PULL-001', file: 'pages/pull-to-refresh.test.ts' },
  { code: 'BRIDGE_STREAM_CLOSED', id: 'CHANNEL-ERROR-001', file: 'pages/channel.test.ts' },
  { code: 'BRIDGE_TOPIC_NOT_FOUND', id: 'CHANNEL-ERROR-001', file: 'pages/channel.test.ts' },
];
