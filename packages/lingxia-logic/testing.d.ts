import type { Lx } from "@lingxia/types";
import type { PageApi, PageDefinition } from "@lingxia/types/page";

/** Replaces the `lx` that Logic modules imported from `@lingxia/logic` see, in a Node test. */
export declare function setLx(value: Lx | undefined): void;

type ParamsOf<D> = D extends PageDefinition<any, any, infer P, any, any> ? P : never;

/** How `mount` opens the page. */
export interface MountOptions<Params> {
  /** What the page is opened with; `onLoad` and `page.params` receive it. */
  params?: Params;
  /** The `lx` the page sees, such as one from `createLxMock`; restored by `unmount`. */
  lx?: Lx;
  /**
   * The page name `page.name` reports.
   * @default 'page'
   */
  name?: string;
}

/** A page `mount` opened in Node. */
export interface MountedPage<D extends PageDefinition<any, any, any, any, any>> {
  /** The page's current `data`, after every `page.update` so far. */
  readonly data: D["data"];
  /** The page's actions, called as the View calls them. */
  readonly actions: D["actions"];
  /** The object `setup` received. */
  readonly page: D extends PageDefinition<infer Data, any, infer P, infer R, infer M> ? PageApi<Data, P, R, M> : never;
  /** Run `onShow`. */
  show(): Promise<void>;
  /** Run `onHide`. */
  hide(): Promise<void>;
  /** Run `onUnload`, abort `page.signal`, and put back the `lx` that was there before. */
  unmount(): Promise<void>;
}

/**
 * Run a `definePage` page in Node with the real page runtime: `setup`, then
 * `onLoad`, `onShow` and `onReady`. The View is not rendered; `page.update`
 * changes `data` at once.
 * @example
 * ```ts
 * const mounted = await mount(page, { params: { id: '7' }, lx: createLxMock({ pick: { file } }) });
 * await mounted.actions.attach();
 * ```
 */
export declare function mount<D extends PageDefinition<any, any, any, any, any>>(
  definition: D,
  options?: MountOptions<ParamsOf<D>>,
): Promise<MountedPage<D>>;
