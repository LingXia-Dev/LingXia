import type { Lx } from "@lingxia/types";

/** Replaces the `lx` that Logic modules imported from `@lingxia/logic` see, in a Node test. */
export declare function setLx(value: Lx | undefined): void;
