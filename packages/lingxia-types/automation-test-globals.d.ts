/// <reference path="./logic-globals.d.ts" />

import type { Automation } from './automation/index.js';

/** Globals exposed by the `lxdev test` JavaScript runtime. */
interface AutomationTestLx {
  automation(): Automation;
}

declare global {
  const lx: AutomationTestLx;
}

export {};
