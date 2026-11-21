// Owners are repository-relative executable tests. Normalization/envelope
// coverage does not imply that every native business failure was induced.
export const families = [
  {
    family: 'LxErrorCode', layer: 'normalization',
    owner: 'packages/lingxia-types/tests/error-runtime.mjs',
    codes: ['E_ABORT', 'E_ALREADY_EXISTS', 'E_CAPABILITY_MISSING', 'E_INTERNAL', 'E_INVALID_ARG',
      'E_INVALID_DATA', 'E_INVALID_STATE', 'E_IO', 'E_NETWORK', 'E_NOT_FOUND', 'E_NOT_SUPPORTED',
      'E_OUT_OF_RANGE', 'E_PERMISSION_DENIED', 'E_QUOTA', 'E_STREAM', 'E_SURFACE_CLOSED',
      'E_SURFACE_CONFLICT', 'E_SURFACE_ROLE_LOCKED', 'E_TIMEOUT', 'E_TYPE'],
    remaining: 'Native production of every code needs individual API, permission, network, and host-version fixtures.',
  },
  {
    family: 'SurfaceErrorCode', layer: 'normalization',
    owner: 'packages/lingxia-types/tests/error-runtime.mjs',
    codes: ['unsupported_placement', 'denied', 'not_declared', 'invalid_arg',
      'already_open_other_role', 'role_locked', 'closed', 'capability_missing', 'failed'],
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
  { code: 'E_INVALID_STATE', id: 'PULL-001', file: 'packages/lingxia-test/showcase/pages/pull-to-refresh.test.ts' },
  { code: 'BRIDGE_STREAM_CLOSED', id: 'CHANNEL-ERROR-001', file: 'packages/lingxia-test/showcase/pages/channel.test.ts' },
  { code: 'BRIDGE_TOPIC_NOT_FOUND', id: 'CHANNEL-ERROR-001', file: 'packages/lingxia-test/showcase/pages/channel.test.ts' },
];
