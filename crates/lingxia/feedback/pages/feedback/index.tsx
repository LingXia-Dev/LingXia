// The window feedback layer: `lx.ui.toast` and its dialogs, drawn over every LingXia surface in a
// window. Rust owns the queue and the words; this page draws its `data` and answers with `answer`.
import { ActionSheet } from '@liulidesign/core/ActionSheet';
import { Button } from '@liulidesign/core/Button';
import { Dialog } from '@liulidesign/core/Dialog';
import { Toast } from '@liulidesign/core/Toast';

type ToastData = { key: number; title: string; tone: 'neutral' | 'success' | 'danger'; loading: boolean; position: 'top' | 'center' | 'bottom' };
type DialogData =
  | { key: number; kind: 'alert'; title: string; message: string; confirm: string; destructive: boolean }
  | { key: number; kind: 'confirm'; title: string; message: string; confirm: string; cancel: string; destructive: boolean }
  | { key: number; kind: 'sheet'; items: string[]; cancel: string; label: string; destructive: boolean };
type Choice = 'confirm' | 'cancel' | number;
type View = {
  data: { toast: ToastData | null; dialog: DialogData | null };
  actions: { answer: (answer: { key: number; choice: Choice }) => Promise<null> };
};

const place = { top: 'justify-start pt-20', center: 'justify-center', bottom: 'justify-end pb-20' } as const;

function toastLayer(toast: ToastData) {
  return (
    <view key={`toast-${toast.key}`} className={`absolute inset-0 items-center px-4 ${place[toast.position]}`}>
      <view role="status" aria-live="polite" aria-label={toast.title}
        className="transition duration-base ease-standard starting:opacity-0 exit:opacity-0">
        <Toast message={toast.title} tone={toast.tone} isBusy={toast.loading} data-testid="lx-toast" />
      </view>
    </view>
  );
}

function dialogLayer(dialog: DialogData, answer: (choice: Choice) => void) {
  const variant = dialog.destructive ? 'destructive' : 'primary';
  if (dialog.kind === 'sheet') {
    // The sheet closes before an item's onClick runs: the cancel waits a turn so a choice wins.
    let chosen = false;
    const choose = (index: number) => () => { chosen = true; answer(index); };
    const cancel = () => { void Promise.resolve().then(() => { if (!chosen) answer('cancel'); }); };
    return (
      <ActionSheet key={`sheet-${dialog.key}`} isOpen onOpenChange={cancel} label={dialog.label} isLabelHidden
        cancelLabel={dialog.cancel} data-testid="lx-action-sheet"
        items={dialog.items.map((item, index) => ({
          id: `item-${index}`, label: item, variant: dialog.destructive ? 'destructive' : 'default',
          onClick: choose(index), 'data-testid': `lx-action-sheet-item-${index}`,
        }))} />
    );
  }
  // Escape and back cancel; an alert's only answer is its button.
  const cancel = () => answer(dialog.kind === 'alert' ? 'confirm' : 'cancel');
  return (
    <Dialog key={`modal-${dialog.key}`} isOpen onOpenChange={cancel} label={dialog.title || dialog.message} isLabelHidden={!dialog.title}
      description={dialog.title && dialog.message ? dialog.message : undefined} isAlert size="sm" data-testid="lx-modal"
      actions={
        <>
          {dialog.kind === 'confirm' && <Button label={dialog.cancel} onClick={() => answer('cancel')} data-testid="lx-modal-cancel" />}
          <Button label={dialog.confirm} variant={variant} onClick={() => answer('confirm')} data-testid="lx-modal-confirm" />
        </>
      } />
  );
}

export function render({ data, actions }: View) {
  const { toast, dialog } = data;
  return (
    <view className="absolute inset-0 bg-transparent">
      {dialog && dialogLayer(dialog, choice => { void actions.answer({ key: dialog.key, choice }); })}
      {toast && toastLayer(toast)}
    </view>
  );
}
