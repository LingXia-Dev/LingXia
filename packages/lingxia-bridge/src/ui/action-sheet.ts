import { isDesktop } from '../runtime-env';

interface ActionSheetParams {
  itemList: string[];
  cancelText?: string;
  itemColor?: string;
}

interface ActionSheetResult {
  tapIndex: number;
}

const ANIMATION_MS = 300;
const STYLE_ID = 'lx-action-sheet-style';

let activeSheet: Promise<ActionSheetResult> | null = null;

function isDesktopMode(): boolean {
  return isDesktop();
}

function ensureActionSheetStyle(): void {
  if (document.getElementById(STYLE_ID)) return;
  const style = document.createElement('style');
  style.id = STYLE_ID;
  style.textContent = `
.lx-as-backdrop {
  position: fixed; inset: 0; z-index: 99999;
  background: rgba(0,0,0,0.4);
  opacity: 0; transition: opacity ${ANIMATION_MS}ms ease;
}
.lx-as-backdrop.lx-as-visible { opacity: 1; }

.lx-as-sheet {
  position: fixed; left: 0; right: 0; bottom: 0; z-index: 100000;
  transform: translateY(100%);
  transition: transform ${ANIMATION_MS}ms ease;
  font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Segoe UI', Roboto, sans-serif;
  -webkit-font-smoothing: antialiased;
}
.lx-as-sheet.lx-as-visible { transform: translateY(0); }
.lx-as-sheet.lx-as-desktop {
  left: 50%;
  right: auto;
  bottom: auto;
  top: 50%;
  width: min(92vw, 460px);
  transform: translate(-50%, calc(-50% + 16px));
}
.lx-as-sheet.lx-as-desktop.lx-as-visible { transform: translate(-50%, -50%); }

.lx-as-container {
  background: #fff;
  border-radius: 16px 16px 0 0;
  overflow: hidden;
}
.lx-as-container.lx-as-desktop {
  border-radius: 16px;
}

.lx-as-item {
  display: block; width: 100%;
  height: 56px;
  line-height: 56px;
  padding: 0;
  border: none; background: none;
  font-size: 18px; font-weight: 400;
  text-align: center;
  cursor: pointer;
  -webkit-tap-highlight-color: transparent;
  position: relative;
}
.lx-as-item:active { background: rgba(0,0,0,0.08); }
.lx-as-item + .lx-as-item::before {
  content: ''; position: absolute; top: 0; left: 0; right: 0;
  height: 1px; background: #E0E0E0;
}
.lx-as-separator {
  height: 8px;
  background: #F2F2F2;
}
.lx-as-cancel-btn { font-weight: 500; color: #000; }
`;
  document.head.appendChild(style);
}

export function showActionSheet(params: unknown): Promise<ActionSheetResult> {
  const p = params as ActionSheetParams | undefined;
  const items = p?.itemList ?? [];
  const cancelText = p?.cancelText ?? 'Cancel';
  const itemColor = p?.itemColor ?? '#007AFF';

  const task = (activeSheet ?? Promise.resolve()).then(
    () => renderSheet(items, cancelText, itemColor),
    () => renderSheet(items, cancelText, itemColor),
  );

  activeSheet = task;
  return task;
}

function renderSheet(
  items: string[],
  cancelText: string,
  itemColor: string,
): Promise<ActionSheetResult> {
  return new Promise<ActionSheetResult>((resolve) => {
    let dismissed = false;
    const desktopMode = isDesktopMode();
    ensureActionSheetStyle();

    // --- DOM ---
    const backdrop = document.createElement('div');
    backdrop.className = 'lx-as-backdrop';

    const sheet = document.createElement('div');
    sheet.className = 'lx-as-sheet';
    if (desktopMode) {
      sheet.classList.add('lx-as-desktop');
    }

    const container = document.createElement('div');
    container.className = 'lx-as-container';
    if (desktopMode) {
      container.classList.add('lx-as-desktop');
    }

    items.forEach((text, index) => {
      const btn = document.createElement('button');
      btn.className = 'lx-as-item';
      btn.textContent = text;
      btn.style.color = itemColor;
      btn.addEventListener('click', () => dismiss(index));
      container.appendChild(btn);
    });

    const separator = document.createElement('div');
    separator.className = 'lx-as-separator';
    container.appendChild(separator);

    const cancelBtn = document.createElement('button');
    cancelBtn.className = 'lx-as-item lx-as-cancel-btn';
    cancelBtn.textContent = cancelText;
    cancelBtn.addEventListener('click', () => dismiss(-1));
    container.appendChild(cancelBtn);

    sheet.appendChild(container);

    document.body.appendChild(backdrop);
    document.body.appendChild(sheet);

    // Trigger enter animation on next frame
    requestAnimationFrame(() => {
      backdrop.classList.add('lx-as-visible');
      sheet.classList.add('lx-as-visible');
    });

    backdrop.addEventListener('click', () => dismiss(-1));

    function dismiss(tapIndex: number): void {
      if (dismissed) return;
      dismissed = true;
      backdrop.classList.remove('lx-as-visible');
      sheet.classList.remove('lx-as-visible');
      setTimeout(() => {
        backdrop.remove();
        sheet.remove();
        resolve({ tapIndex });
      }, ANIMATION_MS);
    }
  });
}
