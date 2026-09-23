import { spec } from "../../dist/index.js";

/** Registering from a second module is the whole point: `beforeEach` is
 *  file-scoped, and a helper in this file stands in for a second spec file. */
export function registerOtherFileSpec(title) {
  spec(title, async () => {});
}

export function registerOtherFileHook(ran) {
  spec.beforeEach(async () => {
    ran.push("other-file-hook");
  });
}

/** A spec file that also exports a hook helper keeps the hook for itself. */
export function registerOtherFileSpecWithHook(title, ran) {
  spec.beforeEach(async () => {
    ran.push("other-file-hook");
  });
  spec(title, async () => {});
}
