import { initBridge } from './bridge';
import { hasError, getErrorInfo, renderErrorUI } from './error';

export function boot(): void {
  if (hasError()) {
    const errorInfo = getErrorInfo();
    if (errorInfo) {
      renderErrorUI(errorInfo);
    }
    return;
  }

  initBridge();
}

export function bootWhenReady(): void {
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', boot);
  } else {
    boot();
  }
}
