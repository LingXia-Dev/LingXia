/// <reference path="./logic-globals.d.ts" />

import type { HostRunAutomation } from './automation/index.js';

/**
 * Globals exposed by the `lxdev test` JavaScript runtime. The run carries host
 * authority, so `lx.automation()` is the host-run root.
 */
interface AutomationTestLx {
  automation(): HostRunAutomation;
}

declare global {
  const lx: AutomationTestLx;
}

export {};
