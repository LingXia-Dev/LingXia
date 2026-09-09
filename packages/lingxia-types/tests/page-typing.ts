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
