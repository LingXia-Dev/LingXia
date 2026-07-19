/**
 * LingXia TypeScript Definitions
 *
 * Type declarations for the LingXia JS API, driven by Rust implementation.
 */

export * from './generated/logic';
export * from './automation';
export * from './error';
export * from './generated/error';
export * from './generated/i18n';

import './generated/logic';

import type {
  AppConfig,
  AppInstance,
  PageConfig,
  PageInstance,
} from './generated/logic';
import type { Automation } from './automation';

export type Lx = globalThis.Lx;

declare global {
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

  function App(config: AppConfig): AppInstance;
  function getApp<T extends AppInstance = AppInstance>(): T | null;
  function Page<TData extends Record<string, unknown> = Record<string, unknown>>(
    config: PageConfig<TData> & ThisType<PageInstance<TData> & PageConfig<TData>>
  ): void;
  function getCurrentPages<T extends PageInstance = PageInstance>(): T[];
}

export {};
