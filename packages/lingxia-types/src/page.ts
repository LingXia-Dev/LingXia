/**
 * `T` where every value survives the JSON transfer `setData` makes to the
 * View; a function, Date, Map, Set, promise or class instance becomes
 * `never`, so a contract holding one does not compile. `undefined` members
 * are allowed: the transfer drops them, as an optional member reads.
 */
export type JsonData<T> =
  0 extends 1 & T ? any
    : unknown extends T ? unknown
    : T extends string | number | boolean | null | undefined ? T
    : T extends (...args: never[]) => unknown ? never
    : T extends Date | RegExp | Map<unknown, unknown> | Set<unknown> | PromiseLike<unknown> | symbol | bigint ? never
    : T extends readonly unknown[] ? { [K in keyof T]: JsonData<T[K]> }
    : T extends object ? { [K in keyof T]: JsonData<T[K]> }
    : never;

/**
 * A shared, type-only page contract: the page's `data` and public actions,
 * written once and used by Logic, View and tests. Importing it installs no
 * Logic globals. `Data` must be JSON (see `JsonData`); otherwise the
 * contract has no `data`, and every use of it names the problem.
 */
export type PageContract<Data = unknown, Actions = unknown> =
  [Data] extends [JsonData<Data>]
    ? { readonly data: Data; readonly actions: Actions }
    : { readonly __pageDataMustBeJson: "PageContract data must be JSON: no functions, Dates, Maps, Sets or class instances" };
