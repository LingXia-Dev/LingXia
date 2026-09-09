import "../src/index.js";

// `TData` comes from `data` and `TCustom` from everything else, so a page's own
// members are reachable through `this` instead of collapsing to `unknown`.
Page({
  data: {
    count: 0,
    surfaceDemo: { message: "" },
    rows: [] as { name: string }[],
  },

  onLoad() {
    this.setData({ count: 1 });
    // A path key addresses inside `data`; the runtime resolves it, so it stays open.
    this.setData({ "surfaceDemo.message": "ready" });
    this.setData({ "rows[0].name": "first" });
    this.bump();
    const route: string = this.route;
    void route;
  },

  onShow() {
    const current: number = this.data.count;
    void current;
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

// A field initialized to `null` or `[]` declares no shape, so the later fill is
// open; annotating it pins the fill.
Page({
  data: {
    profile: null,
    rows: [],
    note: null as string | null,
  },
  onLoad() {
    this.setData({ profile: { id: 1 } });
    this.setData({ rows: ["first"] });
    this.setData({ note: "ready" });
    // @ts-expect-error an annotated lazy field keeps its declared type
    this.setData({ note: 42 });
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
