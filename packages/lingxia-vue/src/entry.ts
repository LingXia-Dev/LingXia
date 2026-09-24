import { createApp, type Component } from "vue";
import { renderPageFault } from "@lingxia/bridge";
import { whenPageReady } from "@lingxia/page-runtime";

/**
 * Mount a page's View. The CLI's generated entry calls this; pages never do.
 *
 * With `waitForState` — the page has Logic — the View mounts only once the
 * page's first state has arrived, so `useLxPage().data` is whole from the first
 * render. If Logic never delivers it, the page says so instead of staying blank.
 */
export async function mountPage(
  App: Component,
  options: { waitForState: boolean },
): Promise<void> {
  if (options.waitForState) {
    try {
      await whenPageReady();
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      console.error(`[LingXia] ${location.pathname}: ${reason}`);
      renderPageFault(location.pathname, reason);
      return;
    }
  }
  createApp(App).mount("#app");
}
