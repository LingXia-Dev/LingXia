import type { ErrorInfo } from './types';

const ERROR_STYLES = `
* { margin: 0; padding: 0; box-sizing: border-box; }
body {
  font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Segoe UI', Roboto, sans-serif;
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 100vh;
  background: #f5f5f7;
  color: #1d1d1f;
  padding: 20px;
  -webkit-font-smoothing: antialiased;
}
.lx-error {
  text-align: center;
  max-width: 400px;
}
.lx-error-code {
  font-size: 120px;
  font-weight: 700;
  color: #d1d1d6;
  line-height: 1;
  letter-spacing: -4px;
}
.lx-error-title {
  font-size: 21px;
  font-weight: 600;
  margin: 16px 0 8px;
  color: #1d1d1f;
}
.lx-error-desc {
  font-size: 15px;
  color: #86868b;
  line-height: 1.5;
}
.lx-error-path {
  display: inline-block;
  margin-top: 20px;
  padding: 10px 16px;
  background: rgba(0,0,0,0.04);
  border-radius: 8px;
  font-family: 'SF Mono', ui-monospace, Menlo, Monaco, monospace;
  font-size: 13px;
  color: #1d1d1f;
  word-break: break-all;
  max-width: 100%;
}
@media (prefers-color-scheme: dark) {
  body { background: #000; color: #f5f5f7; }
  .lx-error-code { color: #3a3a3c; }
  .lx-error-title { color: #f5f5f7; }
  .lx-error-desc { color: #86868b; }
  .lx-error-path { background: rgba(255,255,255,0.08); color: #f5f5f7; }
}
`;

function escapeHtml(str: string): string {
  const div = document.createElement('div');
  div.textContent = str;
  return div.innerHTML;
}

export function renderErrorUI(errorInfo: ErrorInfo): void {
  const { failedPath, reason } = errorInfo;

  const styleEl = document.createElement('style');
  styleEl.textContent = ERROR_STYLES;
  document.head.appendChild(styleEl);

  document.body.innerHTML = '';
  document.body.style.cssText = '';

  const container = document.createElement('div');
  container.className = 'lx-error';

  let html = '<div class="lx-error-code">404</div>';
  html += '<h1 class="lx-error-title">Page Not Found</h1>';
  html += '<p class="lx-error-desc">The page you\'re looking for doesn\'t exist.</p>';

  if (failedPath) {
    html += `<div class="lx-error-path">${escapeHtml(failedPath)}</div>`;
  }

  if (reason) {
    html += `<p class="lx-error-desc" style="margin-top:12px">${escapeHtml(reason)}</p>`;
  }

  container.innerHTML = html;
  document.body.appendChild(container);
}

export function hasError(): boolean {
  const config = window.__LX_RUNTIME_CONFIG;
  return !!(config && config.error);
}

export function getErrorInfo(): ErrorInfo | null {
  const config = window.__LX_RUNTIME_CONFIG;
  return config?.error || null;
}
