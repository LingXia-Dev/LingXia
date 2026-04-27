/**
 * LingXia Web Runtime
 *
 * Injected into SPA HTML to handle:
 * - Bridge initialization for native communication
 * - Error UI rendering when page cannot be loaded
 */

export type {
  BridgeConfig,
  CallOptions,
  ChannelCloseEvent,
  ChannelOptions,
  ChannelOpenOptions,
  BridgeErrorCode,
  ChannelEventName,
  DataSubscriber,
  ErrorInfo,
  LingXiaBridgeInterface,
  LxChannel,
  LxBridgeError,
  LxMethod,
  LxMethodParams,
  LxMethodResult,
  LxMethodStreamData,
  LxStream,
  NativeComponentMessage,
  NativeChannel,
  NativeError,
  NativeStream,
  NotifyOptions,
  InvokeOptions,
  StateInfo,
  StreamCallOptions,
  StreamOptions,
  StreamEventName,
} from './types';

export {
  LingXiaBridge,
  channel,
  initBridge,
  invoke,
  notify,
  stream,
} from './bridge';
export { isNativeError } from './invocation';
export { renderErrorUI, hasError, getErrorInfo } from './error';
export { boot, bootWhenReady } from './boot';

import { bootWhenReady } from './boot';
import { registerUIHandlers } from './ui';

declare const __LX_RUNTIME_PLATFORM__: 'all' | 'desktop' | 'mobile';

if (__LX_RUNTIME_PLATFORM__ === 'all' || __LX_RUNTIME_PLATFORM__ === 'desktop') {
  registerUIHandlers();
}
bootWhenReady();
