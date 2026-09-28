import { createApp, type Component } from "vue";
import { waitForPageState } from "@lingxia/page-runtime";

/**
 * Mount a page's View. The CLI's generated entry calls this; pages never do.
 *
 * With `waitForState` — the page has Logic — the View mounts only once the
 * page's first state has arrived, so `useLxPage().data` is whole from the first
 * render. If that takes longer than 10 seconds, a panel names the page
 * instead of a blank screen — and if the state arrives after all (a slow cold
 * start, a paused debugger), the panel goes and the page mounts.
 */
export async function mountPage(
  App: Component,
  options: { waitForState: boolean },
): Promise<void> {
  if (options.waitForState) await waitForPageState();
  createApp(App).mount("#app");
}
