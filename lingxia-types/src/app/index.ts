/**
 * App & Page APIs
 * Corresponds to: lingxia-lxapp/src/appservice/
 */

export interface AppLifecycleEventArgs {
  source: 'host' | 'lxapp';
  reason:
    | 'foreground'
    | 'background'
    | 'screenshot'
    | 'open'
    | 'close'
    | 'switch_back'
    | 'switch_away';
}

export interface AppLaunchOptions {
  path?: string;
  query?: Record<string, string>;
  scene?: number;
  referrerInfo?: {
    appId?: string;
    extraData?: Record<string, unknown>;
  };
}

export interface AppConfig {
  globalData?: Record<string, unknown>;
  onLaunch?: (options?: AppLaunchOptions) => void | Promise<void>;
  onShow?: (args?: AppLifecycleEventArgs) => void | Promise<void>;
  onHide?: (args?: AppLifecycleEventArgs) => void | Promise<void>;
  onUserCaptureScreen?: () => void | Promise<void>;
  [key: string]: unknown;
}

export interface AppInstance extends AppConfig {
  globalData: Record<string, unknown>;
}

export interface PageLoadOptions {
  [key: string]: string | undefined;
}

export interface PageConfig<TData extends Record<string, unknown> = Record<string, unknown>> {
  data?: TData;
  onLoad?: (options?: PageLoadOptions) => void | Promise<void>;
  onShow?: () => void | Promise<void>;
  onReady?: () => void | Promise<void>;
  onHide?: () => void | Promise<void>;
  onUnload?: () => void | Promise<void>;
  onPullDownRefresh?: () => void | Promise<void>;
  [key: string]: unknown;
}

export interface PageInstance<TData extends Record<string, unknown> = Record<string, unknown>> {
  data: TData;
  route: string;
  setData(data: Partial<TData> | Record<string, unknown>, callback?: () => void): void;
  getEventEmitter(): EventEmitter;
}

export interface EventEmitter {
  on(event: string, handler: (...args: unknown[]) => void): void;
  off(event: string, handler: (...args: unknown[]) => void): void;
  emit(event: string, ...args: unknown[]): void;
  once(event: string, handler: (...args: unknown[]) => void): void;
}
