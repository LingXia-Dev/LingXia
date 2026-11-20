import { test } from "../../dist/index.js";

/** A shared helper that declares no specs of its own: its hooks belong to the
 *  spec file that calls it. */
export function installHooks(ran) {
  test.reset(async () => {
    ran.push("reset");
  });
  test.beforeEach(async () => {
    ran.push("beforeEach");
  });
  test.afterEach(async () => {
    ran.push("afterEach");
  });
}
