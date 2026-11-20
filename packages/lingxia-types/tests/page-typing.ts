import type { DeepReadonly } from "../src/index.js";
import "../src/index.js";

type NestedReadonlyView = DeepReadonly<{ nested: { count: number } }>["nested"];
function nestedViewIsReadonly(view: NestedReadonlyView): void {
  // @ts-expect-error DeepReadonly must block nested writes
  view.count = 2;
}
void nestedViewIsReadonly;

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
