import { spec } from "../../dist/index.js";

/** A shared helper that declares no specs of its own: its hooks belong to the
 *  spec file that calls it. */
export function installHooks(ran) {
  spec.reset(async () => {
    ran.push("reset");
  });
  spec.beforeEach(async () => {
    ran.push("beforeEach");
  });
  spec.afterEach(async () => {
    ran.push("afterEach");
  });
}
