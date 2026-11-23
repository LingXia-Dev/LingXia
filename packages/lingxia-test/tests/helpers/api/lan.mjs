import { test } from "../../../dist/index.js";

/** Stands in for a spec file that shares its name with one in another folder. */
export function registerLanSpec(title) {
  test(title, async () => {});
}
