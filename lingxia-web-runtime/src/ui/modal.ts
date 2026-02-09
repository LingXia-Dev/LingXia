interface ModalParams {
  title?: string;
  content?: string;
  showCancel?: boolean;
  cancelText?: string;
  cancelColor?: string;
  confirmText?: string;
  confirmColor?: string;
}

interface ModalResult {
  confirm: boolean;
  cancel: boolean;
}

const ANIMATION_MS = 200;
const STYLE_ID = 'lx-modal-style';

let activeModal: Promise<ModalResult> | null = null;

function ensureModalStyle(): void {
  if (document.getElementById(STYLE_ID)) return;
  const style = document.createElement('style');
  style.id = STYLE_ID;
  style.textContent = `
.lx-modal-backdrop {
  position: fixed; inset: 0; z-index: 99999;
  background: rgba(0,0,0,0.4);
  opacity: 0; transition: opacity ${ANIMATION_MS}ms ease;
  display: flex; align-items: center; justify-content: center;
}
.lx-modal-backdrop.lx-modal-visible { opacity: 1; }

.lx-modal-dialog {
  width: min(92vw, 320px);
  background: #fff;
  border-radius: 14px;
  overflow: hidden;
  font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Segoe UI', Roboto, sans-serif;
  -webkit-font-smoothing: antialiased;
  transform: scale(0.95); opacity: 0;
  transition: transform ${ANIMATION_MS}ms ease, opacity ${ANIMATION_MS}ms ease;
}
.lx-modal-backdrop.lx-modal-visible .lx-modal-dialog {
  transform: scale(1); opacity: 1;
}

.lx-modal-body {
  padding: 24px 20px 16px;
  text-align: center;
}
.lx-modal-title {
  margin: 0 0 8px;
  font-size: 17px;
  font-weight: 600;
  color: #000;
  line-height: 1.35;
}
.lx-modal-content {
  margin: 0;
  font-size: 14px;
  color: #666;
  line-height: 1.5;
  word-break: break-word;
}
.lx-modal-body:has(.lx-modal-title:empty) .lx-modal-content {
  font-size: 17px;
  font-weight: 600;
  color: #000;
}

.lx-modal-footer {
  display: flex;
  border-top: 1px solid #E0E0E0;
}
.lx-modal-btn {
  flex: 1;
  height: 44px;
  line-height: 44px;
  padding: 0;
  border: none;
  background: none;
  font-size: 17px;
  text-align: center;
  cursor: pointer;
  -webkit-tap-highlight-color: transparent;
}
.lx-modal-btn:active { background: rgba(0,0,0,0.06); }
.lx-modal-btn + .lx-modal-btn {
  border-left: 1px solid #E0E0E0;
}
.lx-modal-btn-cancel {
  color: #666;
  font-weight: 400;
}
.lx-modal-btn-confirm {
  font-weight: 600;
}

@media (prefers-color-scheme: dark) {
  .lx-modal-dialog { background: #2c2c2e; }
  .lx-modal-title { color: #f5f5f7; }
  .lx-modal-content { color: #ababab; }
  .lx-modal-body:has(.lx-modal-title:empty) .lx-modal-content {
    color: #f5f5f7;
  }
  .lx-modal-footer { border-top-color: #3a3a3c; }
  .lx-modal-btn:active { background: rgba(255,255,255,0.08); }
  .lx-modal-btn + .lx-modal-btn { border-left-color: #3a3a3c; }
  .lx-modal-btn-cancel { color: #ababab; }
}
`;
  document.head.appendChild(style);
}

export function showModal(params: unknown): Promise<ModalResult> {
  const p = params as ModalParams | undefined;
  const title = p?.title ?? '';
  const content = p?.content ?? '';
  const showCancel = p?.showCancel ?? true;
  const cancelText = p?.cancelText ?? 'Cancel';
  const cancelColor = p?.cancelColor;
  const confirmText = p?.confirmText ?? 'OK';
  const confirmColor = p?.confirmColor ?? '#007AFF';

  const task = (activeModal ?? Promise.resolve()).then(
    () => renderModal(title, content, showCancel, cancelText, cancelColor, confirmText, confirmColor),
    () => renderModal(title, content, showCancel, cancelText, cancelColor, confirmText, confirmColor),
  );

  activeModal = task;
  return task;
}

function renderModal(
  title: string,
  content: string,
  showCancel: boolean,
  cancelText: string,
  cancelColor: string | undefined,
  confirmText: string,
  confirmColor: string,
): Promise<ModalResult> {
  return new Promise<ModalResult>((resolve) => {
    let dismissed = false;
    ensureModalStyle();

    const backdrop = document.createElement('div');
    backdrop.className = 'lx-modal-backdrop';

    const dialog = document.createElement('div');
    dialog.className = 'lx-modal-dialog';

    // Body
    const body = document.createElement('div');
    body.className = 'lx-modal-body';

    const titleEl = document.createElement('div');
    titleEl.className = 'lx-modal-title';
    titleEl.textContent = title;
    body.appendChild(titleEl);

    if (content) {
      const contentEl = document.createElement('p');
      contentEl.className = 'lx-modal-content';
      contentEl.textContent = content;
      body.appendChild(contentEl);
    }

    dialog.appendChild(body);

    // Footer
    const footer = document.createElement('div');
    footer.className = 'lx-modal-footer';

    if (showCancel) {
      const cancelBtn = document.createElement('button');
      cancelBtn.className = 'lx-modal-btn lx-modal-btn-cancel';
      cancelBtn.textContent = cancelText;
      if (cancelColor) cancelBtn.style.color = cancelColor;
      cancelBtn.addEventListener('click', () => dismiss(false));
      footer.appendChild(cancelBtn);
    }

    const confirmBtn = document.createElement('button');
    confirmBtn.className = 'lx-modal-btn lx-modal-btn-confirm';
    confirmBtn.textContent = confirmText;
    confirmBtn.style.color = confirmColor;
    confirmBtn.addEventListener('click', () => dismiss(true));
    footer.appendChild(confirmBtn);

    dialog.appendChild(footer);
    backdrop.appendChild(dialog);
    document.body.appendChild(backdrop);

    // Trigger enter animation
    requestAnimationFrame(() => {
      backdrop.classList.add('lx-modal-visible');
    });

    function dismiss(confirmed: boolean): void {
      if (dismissed) return;
      dismissed = true;
      backdrop.classList.remove('lx-modal-visible');
      setTimeout(() => {
        backdrop.remove();
        resolve({ confirm: confirmed, cancel: !confirmed });
      }, ANIMATION_MS);
    }
  });
}
