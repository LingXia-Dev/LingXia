import type { Lx } from "@lingxia/types";

export * from "@lingxia/types";
export * from "@lingxia/types/page";

/**
 * The runtime API. Import it in Logic: `import { defineApp, definePage, lx } from '@lingxia/logic'`.
 * A View cannot use it; it calls an action that does.
 */
export declare const lx: Lx;

/** A component library's words, as `lingxia gen i18n` compiles them: `(args, formatter) => text` per key. */
export interface PackageWords {
  readonly source: string;
  readonly languages: Readonly<Record<string, Readonly<Record<string, (args: any, formatter: any) => unknown>>>>;
  formatter(tag: string): unknown;
}
/**
 * A component library's word `key` (`@jinshu.agent.edit`) in `language`, for Logic (an agent tool's
 * summary, a status line): the lxapp's own catalogue entry for the key in that language when it has
 * one, else the package's, else the package's source language.
 */
export declare function packageText(words: PackageWords, key: string, args: Readonly<Record<string, unknown>> | undefined, language: string): string;
