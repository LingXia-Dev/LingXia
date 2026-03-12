import { initBridge } from './bridge';
import { hasError, getErrorInfo, renderErrorUI } from './error';

const domReadyListener: EventListenerObject = {
  handleEvent: () => maybeRenderErrorUI(),
};

function maybeRenderErrorUI(): void {
  if (hasError()) {
    const errorInfo = getErrorInfo();
    if (errorInfo) {
      renderErrorUI(errorInfo);
    }
  }
}

/**
 * Immediately initialize bridge and render error UI if present.
 * Use this when DOM is already ready.
 */
export function boot(): void {
  initBridge();
  maybeRenderErrorUI();
}

/**
 * Initialize bridge immediately, but wait for DOM to render error UI.
 * Use this at page load time.
 */
export function bootWhenReady(): void {
  initBridge();
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', domReadyListener);
  } else {
    maybeRenderErrorUI();
  }
}
