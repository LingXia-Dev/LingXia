import type { Lx } from "@lingxia/types";

export * from "@lingxia/types";
export * from "@lingxia/types/page";

/**
 * The runtime API. Import it in Logic: `import { definePage, lx } from '@lingxia/logic'`.
 * A View cannot use it; it calls an action that does.
 */
export declare const lx: Lx;
