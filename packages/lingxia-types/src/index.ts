/**
 * LingXia TypeScript Definitions
 *
 * Type declarations for the LingXia JS API, driven by Rust implementation.
 */

export * from './generated/logic.js';
export type { Automation } from './automation/index.js';
export * from './error.js';
export * from './generated/error.js';
export * from './generated/i18n.js';

import './generated/logic.js';

import type {
  AppConfig,
  AppInstance,
  AppLifecycleName,
  BinaryFileData,
  FsWriteOptions,
  NoLifecycleTypos,
  PageConfig,
  PageInstance,
  PageLifecycleName,
} from './generated/logic.js';
import type { Automation } from './automation/index.js';

export type Lx = globalThis.Lx;

declare global {
  interface FileSystemApi {
    /**
     * Write bytes to a managed file. `encoding` describes how to read a
     * *string*, so it has no meaning here and the runtime rejects it — this
     * overload is what makes that a compile error instead. The generated
     * signature above covers the string case.
     */
    write(
      path: string,
      data: BinaryFileData,
      options?: Omit<FsWriteOptions, 'encoding'>
    ): Promise<void>;
  }

  interface Lx {
    /**
     * In-process UI/runtime automation.
     *
     * Select the current app with `.lxapp()` or a specific running app with
     * `.lxapp(appid)`. Host-only surfaces enforce the `host` privilege when
     * selected; `lingxia dev` and the Runner grant it implicitly.
     */
    automation(): Automation;
  }

  const lx: Lx;

  /**
   * `TCustom` is inferred from the members you write beside the lifecycle
   * hooks, so `this.myMethod()` resolves inside the config and a key that is
   * only a case away from a hook is rejected.
   */
  function App<TCustom>(
    config: AppConfig &
      TCustom &
      NoLifecycleTypos<TCustom, AppLifecycleName> &
      ThisType<AppInstance & TCustom>
  ): AppInstance & TCustom;
  function getApp<T extends AppInstance = AppInstance>(): T | null;
  /**
   * `TData` comes from `data`, `TCustom` from everything else you declare, so
   * `this.data` is typed, `this.myMethod()` resolves, and `setData` checks
   * top-level keys against `data`.
   */
  function Page<TData extends Record<string, unknown>, TCustom>(
    config: PageConfig<TData> &
      TCustom &
      NoLifecycleTypos<TCustom, PageLifecycleName> &
      ThisType<PageInstance<TData> & TCustom>
  ): void;
  function getCurrentPages<T extends PageInstance = PageInstance>(): T[];
}

export {};
