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
  SameLevelMessage,
} from './types';

export { LingXiaBridge, lx, initBridge } from './bridge';
export { renderErrorUI, hasError, getErrorInfo } from './error';
export { boot, bootWhenReady } from './boot';

import { bootWhenReady } from './boot';
bootWhenReady();
