// The catalogue's root: what `import { lx } from '@lingxia/logic'` gives Logic.
import type { Lx } from "../src/index.js";

declare global {
  const lx: Lx;
}
export {};
