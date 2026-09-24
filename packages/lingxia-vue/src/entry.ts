import { createApp, type Component } from "vue";
import { renderPageFault } from "@lingxia/bridge";
import { whenPageReady } from "@lingxia/page-runtime";

const FAULT_AFTER_MS = 10_000;

async function waitForPageState(): Promise<void> {
  // Set from the timer's callback, so held where control flow cannot narrow it.
  const panel: { dismiss?: () => void } = {};
  const fault = setTimeout(() => {
    const reason = `Page Logic has not delivered the page state after ${FAULT_AFTER_MS} ms`;
    console.error(`[LingXia] ${location.pathname}: ${reason}`);
    panel.dismiss = renderPageFault(location.pathname, reason);
  }, FAULT_AFTER_MS);
  await whenPageReady({ timeoutMs: null });
  clearTimeout(fault);
  panel.dismiss?.();
}

/**
 * Mount a page's View. The CLI's generated entry calls this; pages never do.
 *
 * With `waitForState` — the page has Logic — the View mounts only once the
 * page's first state has arrived, so `useLxPage().data` is whole from the first
 * render. If that takes longer than `FAULT_AFTER_MS`, a panel names the page
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
