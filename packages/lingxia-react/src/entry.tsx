import * as React from "react";
import { createRoot } from "react-dom/client";
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
  App: React.ComponentType,
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
  const root = document.getElementById("root");
  if (!root) throw new Error("The page document has no #root element to mount into");
  createRoot(root).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}
