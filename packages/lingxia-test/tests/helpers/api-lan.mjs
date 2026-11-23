import { test } from "../../dist/index.js";

/** Its path below `tests/` reads the same as `helpers/api/lan`'s once folders become dashes. */
export function registerLanSpec(title) {
  test(title, async () => {});
}
