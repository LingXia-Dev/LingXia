import { renderPageFault } from "@lingxia/bridge";
import { isPageReady, whenPageReady } from "./runtime.js";

let pending: Promise<void> | undefined;

/** Mount with complete initial data; a delayed startup stays recoverable. */
export function waitForPageState(): Promise<void> {
  if (isPageReady()) return Promise.resolve();
  return pending ??= wait().finally(() => { pending = undefined; });
}

async function wait(): Promise<void> {
  const delay = 10_000;
  const panel: { dismiss?: () => void } = {};
  const timer = setTimeout(() => {
    const reason = `Page Logic has not delivered the page state after ${delay} ms`;
    console.error(`[LingXia] ${location.pathname}: ${reason}`);
    panel.dismiss = renderPageFault(location.pathname, reason);
  }, delay);
  try {
    await whenPageReady({ timeoutMs: null });
  } finally {
    clearTimeout(timer);
    panel.dismiss?.();
  }
}
