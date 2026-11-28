/// <reference path="./generated/logic-web.d.ts" />
// What a Logic program sees without importing. A View program never lists this entry, so `lx`
// there is "cannot find name"; in Logic it is a type whose only member names the import.

declare global {
  /** Import it: `import { lx } from '@lingxia/logic'`. */
  const lx: { readonly "import { lx } from '@lingxia/logic'": never };
}

export {};
