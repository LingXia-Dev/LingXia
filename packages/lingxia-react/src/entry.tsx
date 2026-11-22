import * as React from "react";
import { createRoot } from "react-dom/client";
import { waitForPageState } from "@lingxia/page-runtime";
import { registerCatalogues, type CompiledCatalogues } from "@lingxia/bridge";

/**
 * Mount a page's View. The CLI's generated entry calls this; pages never do.
 *
 * With `waitForState` — the page has Logic — the View mounts only once the
 * page's first state has arrived, so `useLxPage().data` is whole from the first
 * render. If that takes longer than 10 seconds, a panel names the page
 * instead of a blank screen — and if the state arrives after all (a slow cold
 * start, a paused debugger), the panel goes and the page mounts.
 *
 * `messages` is the lxapp's compiled `i18n/`, registered before the first
 * render so `useText()` works there.
 */
export async function mountPage(
  App: React.ComponentType,
  options: { waitForState: boolean; messages?: CompiledCatalogues },
): Promise<void> {
  if (options.messages) registerCatalogues(options.messages);
  if (options.waitForState) await waitForPageState();
  const root = document.getElementById("root");
  if (!root) throw new Error("The page document has no #root element to mount into");
  createRoot(root).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}
