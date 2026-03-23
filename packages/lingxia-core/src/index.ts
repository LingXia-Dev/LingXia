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
  Channel,
  ChannelOpenOptions,
  BridgeErrorCode,
  ChannelEventName,
  DataSubscriber,
  ErrorInfo,
  LingXiaBridgeInterface,
  LxBridgeError,
  LxMethod,
  LxMethodParams,
  LxMethodResult,
  LxMethodStreamData,
  NativeComponentMessage,
  NotifyOptions,
  StateInfo,
  StreamCallOptions,
  StreamEventName,
  StreamHandle,
  SubscribeOptions,
  Subscription,
  SubscriptionEventName,
} from './types';

export { LingXiaBridge, host, initBridge } from './bridge';
export { renderErrorUI, hasError, getErrorInfo } from './error';
export { boot, bootWhenReady } from './boot';

import { bootWhenReady } from './boot';
import { setupImageProxy } from './image-proxy';
import { registerUIHandlers } from './ui';

declare const __LX_RUNTIME_PLATFORM__: 'all' | 'desktop' | 'mobile';

if (__LX_RUNTIME_PLATFORM__ === 'all' || __LX_RUNTIME_PLATFORM__ === 'desktop') {
  registerUIHandlers();
}
setupImageProxy();
bootWhenReady();
