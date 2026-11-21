/**
 * LingXia TypeScript Definitions
 *
 * Type declarations for the LingXia JS API, driven by Rust implementation.
 */

export * from './generated/logic.js';
export type { Automation } from './automation/index.js';
export type { MockAnswer, MockContext, MockHandler, MockRequest, Mocks } from './mocks.js';
export * from './error.js';
export * from './generated/i18n.js';

import './generated/logic.js';

import type { Automation } from './automation/index.js';

export type Lx = globalThis.Lx;

declare global {
  interface Lx {
    /**
     * In-process UI/runtime automation, as app Logic may use it.
     *
     * Select the calling app with `.lxapp()` (requires the `automation`
     * privilege) or another running app with `.lxapp(appid)`; the host tiers
     * (`lxapps`, `browser`, `shell`, `device`, `desktop`, `terminal`) require
     * `host`, checked when the property is read. A privilege holds only when
     * it is allowed for the app and the native host sealed a session grant
     * for it; `lingxia dev` and the Runner do not widen that. Test-run-only
     * members (`network`, nav `waitUntil: 'ready'`) belong to
     * `HostRunAutomation`, the root of an `lxdev test` program.
     */
    automation(): Automation;
  }
}

export {};
