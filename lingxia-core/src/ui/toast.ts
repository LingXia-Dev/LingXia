interface ToastParams {
  title: string;
  icon?: string;
  image?: string;
  duration?: number;
  mask?: boolean;
  position?: string;
}

const ANIMATION_MS = 200;
const STYLE_ID = 'lx-toast-style';

let currentToast: { dismiss: () => void; timer: ReturnType<typeof setTimeout> | null } | null = null;

const SVG_SUCCESS = `<svg width="36" height="36" viewBox="0 0 36 36" fill="none" xmlns="http://www.w3.org/2000/svg"><circle cx="18" cy="18" r="17" stroke="currentColor" stroke-width="2"/><path d="M11 18l5 5 9-9" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"/></svg>`;
const SVG_ERROR = `<svg width="36" height="36" viewBox="0 0 36 36" fill="none" xmlns="http://www.w3.org/2000/svg"><circle cx="18" cy="18" r="17" stroke="currentColor" stroke-width="2"/><path d="M13 13l10 10M23 13l-10 10" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/></svg>`;
const SVG_LOADING = `<svg width="36" height="36" viewBox="0 0 36 36" fill="none" xmlns="http://www.w3.org/2000/svg"><path d="M18 4a14 14 0 0 1 14 14" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"><animateTransform attributeName="transform" type="rotate" from="0 18 18" to="360 18 18" dur="0.8s" repeatCount="indefinite"/></path></svg>`;

function ensureToastStyle(): void {
  if (document.getElementById(STYLE_ID)) return;
  const style = document.createElement('style');
  style.id = STYLE_ID;
  style.textContent = `
.lx-toast-mask {
  position: fixed; inset: 0; z-index: 99998;
}
.lx-toast-container {
  position: fixed; left: 0; right: 0; z-index: 99999;
  display: flex; justify-content: center;
  pointer-events: none;
}
.lx-toast-container.lx-toast-top { top: 0; padding-top: 80px; }
.lx-toast-container.lx-toast-center { top: 0; bottom: 0; align-items: center; }
.lx-toast-container.lx-toast-bottom { bottom: 0; padding-bottom: 80px; }

.lx-toast-box {
  display: flex; flex-direction: column; align-items: center; gap: 10px;
  min-width: 120px; max-width: 280px;
  padding: 20px;
  background: rgba(0,0,0,0.8);
  border-radius: 12px;
  color: #fff;
  pointer-events: auto;
  transform: scale(0.8); opacity: 0;
  transition: transform ${ANIMATION_MS}ms ease, opacity ${ANIMATION_MS}ms ease;
}
.lx-toast-box.lx-toast-no-icon {
  min-width: auto; min-height: auto;
  padding: 12px 20px;
}
.lx-toast-box.lx-toast-visible {
  transform: scale(1); opacity: 1;
}
.lx-toast-icon {
  width: 36px; height: 36px;
  color: #fff;
  line-height: 0;
}
.lx-toast-title {
  font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Segoe UI', Roboto, sans-serif;
  font-size: 16px; font-weight: 500;
  text-align: center;
  line-height: 1.4;
  word-break: break-word;
  -webkit-font-smoothing: antialiased;
}
`;
  document.head.appendChild(style);
}

export function showToast(params: unknown): Promise<void> {
  const p = params as ToastParams | undefined;
  const title = p?.title ?? '';
  const icon = p?.icon ?? 'none';
  const duration = p?.duration ?? 1500;
  const mask = p?.mask ?? false;
  const position = p?.position ?? 'center';

  // Hide existing toast first
  if (currentToast) {
    currentToast.dismiss();
    currentToast = null;
  }

  ensureToastStyle();

  // Mask (prevents interaction)
  let maskEl: HTMLElement | null = null;
  if (mask) {
    maskEl = document.createElement('div');
    maskEl.className = 'lx-toast-mask';
    document.body.appendChild(maskEl);
  }

  // Container
  const container = document.createElement('div');
  container.className = 'lx-toast-container';
  if (position === 'top') container.classList.add('lx-toast-top');
  else if (position === 'bottom') container.classList.add('lx-toast-bottom');
  else container.classList.add('lx-toast-center');

  // Toast box
  const box = document.createElement('div');
  box.className = 'lx-toast-box';
  const hasIcon = icon !== 'none';
  if (!hasIcon) box.classList.add('lx-toast-no-icon');

  // Icon
  if (hasIcon) {
    const iconEl = document.createElement('div');
    iconEl.className = 'lx-toast-icon';
    if (icon === 'success') iconEl.innerHTML = SVG_SUCCESS;
    else if (icon === 'error') iconEl.innerHTML = SVG_ERROR;
    else if (icon === 'loading') iconEl.innerHTML = SVG_LOADING;
    box.appendChild(iconEl);
  }

  // Title
  const titleEl = document.createElement('div');
  titleEl.className = 'lx-toast-title';
  titleEl.textContent = title;
  box.appendChild(titleEl);

  container.appendChild(box);
  document.body.appendChild(container);

  // Animate in
  requestAnimationFrame(() => {
    box.classList.add('lx-toast-visible');
  });

  let dismissed = false;

  function dismiss(): void {
    if (dismissed) return;
    dismissed = true;
    if (currentToast?.dismiss === dismiss) currentToast = null;
    box.classList.remove('lx-toast-visible');
    if (timer) clearTimeout(timer);
    setTimeout(() => {
      container.remove();
      maskEl?.remove();
    }, ANIMATION_MS);
  }

  let timer: ReturnType<typeof setTimeout> | null = null;
  if (duration > 0) {
    timer = setTimeout(dismiss, duration);
  }

  currentToast = { dismiss, timer };
  return Promise.resolve();
}

export function hideToast(): Promise<void> {
  if (currentToast) {
    currentToast.dismiss();
    currentToast = null;
  }
  return Promise.resolve();
}
