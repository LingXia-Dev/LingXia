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
import type {
  Automation,
  AutomationOptions,
  HostAutomation,
} from './automation';

export type Lx = globalThis.Lx;

declare global {
  interface Lx {
    /**
     * In-process UI/runtime automation of the calling lxapp.
     *
     * Base tier drives the app's own pages, navigation, and self info. The host
     * tier (`{ host: true }`) additionally exposes cross-lxapp management, the
     * host browser tabs, and host-window input. Gated by the `automation` /
     * `host` security privileges; `lingxia dev` and the Runner grant them.
     */
    automation(): Automation;
    automation(options: { host: true }): HostAutomation;
    automation(options?: AutomationOptions): Automation;
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
