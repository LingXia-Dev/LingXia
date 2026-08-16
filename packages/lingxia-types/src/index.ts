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
  BinaryFileData,
  FsWriteOptions,
  PageConfig,
  PageInstance,
} from './generated/logic';
import type { Automation } from './automation';

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

  function App(config: AppConfig): AppInstance;
  function getApp<T extends AppInstance = AppInstance>(): T | null;
  function Page<TData extends Record<string, unknown> = Record<string, unknown>>(
    config: PageConfig<TData> & ThisType<PageInstance<TData> & PageConfig<TData>>
  ): void;
  function getCurrentPages<T extends PageInstance = PageInstance>(): T[];
}

export {};
