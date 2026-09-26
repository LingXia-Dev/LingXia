// A spec that imports product Logic next to the test SDK.
import { spec, rawAutomation } from '@lingxia/test';
import type { HostRunAutomation, LxAppDriver } from '@lingxia/types/automation';
import { hasStorageKey, userDataPath } from './app-logic.js';

spec('app Logic and the test SDK type-check together', async (t) => {
  const path: string = await t.app.logic.eval(({ lx }) => lx.env.USER_DATA_PATH);
  path.toUpperCase();
  // The app's `lx` keeps its own meaning in the product modules a spec imports.
  const local: string = userDataPath();
  const known: Promise<boolean> = hasStorageKey('session');
  void local;
  void known;
});

// The raw automation root is an import, not a second meaning of `lx`.
const root: HostRunAutomation = rawAutomation();
const driver: LxAppDriver = root.lxapp('example');
void driver.network;
