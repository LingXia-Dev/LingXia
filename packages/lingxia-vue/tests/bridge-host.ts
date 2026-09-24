// The page hooks need only the host store from `@lingxia/bridge`; its index
// also boots the bridge, which needs a real document. Bundled in its place for
// tests/page-hooks.mjs.
export { getHost, subscribeHost, type LxHost } from '../../lingxia-bridge/src/host';
export type { LxBridgeError, LxChannel, LxStream } from '../../lingxia-bridge/src/index';
