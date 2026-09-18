import type { DeepReadonly } from "../src/index.js";
import "../src/index.js";

type NestedReadonlyView = DeepReadonly<{ nested: { count: number } }>["nested"];
function nestedViewIsReadonly(view: NestedReadonlyView): void {
  // @ts-expect-error DeepReadonly must block nested writes
  view.count = 2;
}
void nestedViewIsReadonly;

// `TData` comes from `data` and `TCustom` from everything else, so a page's own
// members are reachable through `this` instead of collapsing to `unknown`.
Page({
  data: {
    count: 0,
    surfaceDemo: { message: "" },
    rows: [] as { name: string }[],
    nested: { count: 0 },
  },

  onLoad() {
    this.setData({ count: 1 });
    this.setPath(["surfaceDemo", "message"], "ready");
    this.setPath(["rows", 0, "name"], "first");
    this.setDataPath("nested.count", 1);
    this.bump();
    const route: string = this.route;
    void route;
  },

  onShow() {
    const current: number = this.data.count;
    const nestedCount: number = this.data.nested.count;
    void current;
    void nestedCount;
  },

  bump() {
    this.setData({ count: this.data.count + 1 }, () => {});
  },
});

Page({
  data: { count: 0 },
  onLoad() {
    // @ts-expect-error a top-level key must exist in `data`
    this.setData({ cuont: 1 });
    // @ts-expect-error a top-level key must match its declared value type
    this.setData({ count: "one" });
    this.setDataPath("surfaceDemo.message", "ready");
    // @ts-expect-error data is a readonly view
    this.data.count = 2;
  },
});

Page({
  data: { nested: { count: 0 } },
  onLoad() {
    this.data.nested.count + 0;
    // @ts-expect-error setPath checks the value against the path
    this.setPath(["nested", "count"], "wrong");
    // @ts-expect-error setPath checks the path against data
    this.setPath(["typo", "value"], 1);
  },
});

Page({
  data: { count: 0 },
  // @ts-expect-error 'onload' differs from `onLoad` only in case
  onload() {},
});

Page({
  data: { count: 0 },
  // A name that is not a near-miss stays an ordinary page method.
  loadRows() {},
});

// `null` and `[]` keep their initializer types. Annotate the field when a
// later fill should be checked against a wider shape.
Page({
  data: {
    profile: null,
    rows: [],
    note: null as string | null,
  },
  onLoad() {
    // @ts-expect-error a null field stays null until it is annotated
    this.setData({ profile: { id: 1 } });
    // @ts-expect-error an empty array stays never[] until it is annotated
    this.setData({ rows: ["first"] });
    this.setData({ note: "ready" });
    // @ts-expect-error an annotated lazy field keeps its declared type
    this.setData({ note: 42 });
    const profile: null = this.data.profile;
    void profile;
  },
});

// Naming `data` with a type argument is not supported — annotate `data`, which
// keeps the rest of the config inferred.
type CheckoutData = { total: number };
Page({
  data: { total: 0 } as CheckoutData,
  checkout() {
    this.setData({ total: this._sum([1, 2]) });
  },
  _sum(values: number[]) {
    return values.reduce((a, b) => a + b, 0);
  },
});

App({
  globalData: { session: "" },
  onLaunch() {
    this.refresh();
  },
  refresh() {},
});

App({
  // @ts-expect-error 'onlaunch' differs from `onLaunch` only in case
  onlaunch() {},
});

const pages = getCurrentPages();
const firstRoute: string | undefined = pages[0]?.route;
void firstRoute;

export type PageTypingGate = [typeof firstRoute];
