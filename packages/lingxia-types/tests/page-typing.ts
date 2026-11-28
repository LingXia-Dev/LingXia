import type { DeepReadonly } from "../src/index.js";
import "../src/index.js";
import { defineApp } from "../src/page.js";

type NestedReadonlyView = DeepReadonly<{ nested: { count: number } }>["nested"];
function nestedViewIsReadonly(view: NestedReadonlyView): void {
  // @ts-expect-error DeepReadonly must block nested writes
  view.count = 2;
}
void nestedViewIsReadonly;

defineApp({
  onLaunch() {
    this.refresh();
  },
  refresh() {},
});

defineApp({
  // @ts-expect-error 'onlaunch' differs from `onLaunch` only in case
  onlaunch() {},
});

// App state lives in modules; the page stack is reached through `page`.
// @ts-expect-error getCurrentPages is gone
getCurrentPages;
// @ts-expect-error getApp is gone
getApp;
// @ts-expect-error global App() is gone
App;

defineApp({
  onOpen(link) {
    const from: string = link.url;
    if (link.query.id) return { page: 'detail', params: { id: link.query.id } };
    return from ? { tab: 'home' } : undefined;
  },
});

defineApp({
  // @ts-expect-error onOpen returns a navigation target, not a page name
  onOpen: () => 'detail',
});
