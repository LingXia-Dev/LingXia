/**
 * LingXia Web Runtime
 *
 * Injected into SPA HTML to handle:
 * - Bridge initialization for native communication
 * - Error UI rendering when page cannot be loaded
 */

export type {
  BridgeConfig,
  ErrorInfo,
  LingXiaBridgeInterface,
  DataSubscriber,
  NativeComponentMessage,
} from './types';

export { LingXiaBridge, lx, initBridge } from './bridge';
export { renderErrorUI, hasError, getErrorInfo } from './error';
export { boot, bootWhenReady } from './boot';

import { bootWhenReady } from './boot';
import { setupImageProxy } from './image-proxy';
setupImageProxy();
bootWhenReady();
