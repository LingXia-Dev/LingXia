import type { Lx } from "@lingxia/types";
import type { PageApi, PageDefinition } from "@lingxia/types/page";

export { createLxMock } from "@lingxia/types/testing";
export type { LxMockOverrides } from "@lingxia/types/testing";

/** Replaces the `lx` that Logic modules imported from `@lingxia/logic` see, in a Node test. */
export declare function setLx(value: Lx | undefined): void;

type Declared<D> = D extends { readonly __lxPage: infer P } ? P : never;
type ParamsOf<D> = Declared<D> extends { readonly params: infer P } ? P : never;
type ActionsOf<D> = Declared<D> extends { readonly actions: infer A } ? A : never;
type DataOf<D> = D extends { readonly data: infer Data } ? Data : never;
type ApiOf<D> = Declared<D> extends { readonly params: infer P; readonly result: infer R; readonly messages: infer M }
  ? PageApi<DataOf<D>, P, R, M>
  : never;

/** How `mount` opens the page. */
export interface MountOptions<Params> {
  /** What the page is opened with; `onLoad` and `page.params` receive it. */
  params?: Params;
  /**
   * The `lx` the page sees, such as one from `createLxMock`; restored by `unmount`. Pages mounted
   * at the same time share one `lx`: a second with a different one throws.
   */
  lx?: Lx;
  /**
   * The page name `page.name` reports.
   * @default 'page'
   */
  name?: string;
  /**
   * `'allow'` lets `unmount` resolve with timers the page left running; rare, for a test about
   * something else. They are stopped either way.
   * @default 'fail'
   */
  leakedTimers?: "fail" | "allow";
}

/** A page `mount` opened in Node. */
export interface MountedPage<D extends PageDefinition> {
  /** The page's current `data`, after every `page.update` so far. */
  readonly data: DataOf<D>;
  /** The page's actions as it wrote them: a sync action returns, an async generator streams. */
  readonly actions: ActionsOf<D>;
  /** The object `setup` received. */
  readonly page: ApiOf<D>;
  /** Run `onShow`; `{ params }` is a tab switch with params, as `lx.nav.go({ tab, params })` hands them over. */
  show(options?: { params?: ParamsOf<D> }): Promise<void>;
  /** Run `onHide`. */
  hide(): Promise<void>;
  /**
   * Abort `page.signal`, run `onUnload`, and put back the `lx` that was there before. A timer the
   * page started and left running leaks in the app, so it is stopped and `unmount` rejects naming
   * where it started. A second call does nothing.
   */
  unmount(): Promise<void>;
}

/**
 * Run a `definePage` page in Node with the real page runtime: `setup`, then
 * `onLoad`, `onShow` and `onReady`. The View is not rendered; `page.update`
 * changes `data` at once. If a hook throws, the page is unmounted and the
 * error rethrown.
 * @example
 * ```ts
 * const mounted = await mount(page, { params: { id: '7' }, lx: createLxMock({ pick: { file } }) });
 * await mounted.actions.attach();
 * ```
 */
export declare function mount<D extends PageDefinition>(
  definition: D,
  options?: MountOptions<ParamsOf<D>>,
): Promise<MountedPage<D>>;
