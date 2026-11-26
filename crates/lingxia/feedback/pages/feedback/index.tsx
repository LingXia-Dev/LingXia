// The window feedback layer: `lx.ui.toast` and its dialogs, drawn over every LingXia surface in a
// window. Rust owns the queue and the words; this page draws its `data` and answers with `answer`.
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
      <view role="status" aria-live="polite" aria-label={toast.title} aria-busy={toast.loading}
        className="transition duration-base ease-standard starting:opacity-0 exit:opacity-0">
        <Toast message={toast.title} tone={toast.tone} data-testid="lx-toast" />
      </view>
    </view>
  );
}

function dialogLayer(dialog: DialogData, answer: (choice: Choice) => void) {
  const variant = dialog.destructive ? 'destructive' : 'primary';
  if (dialog.kind === 'sheet') {
    return (
      <Dialog key={`sheet-${dialog.key}`} isOpen onOpenChange={() => answer('cancel')} label={dialog.label} isLabelHidden
        hasCloseButton={false} size="sm" data-testid="lx-action-sheet"
        actions={
          <view className="w-full gap-2">
            {dialog.items.map((item, index) => (
              <Button key={`item-${index}`} label={item} variant={dialog.destructive ? 'destructive' : 'tonal'}
                onClick={() => answer(index)} data-testid={`lx-action-sheet-item-${index}`} />
            ))}
            <Button label={dialog.cancel} onClick={() => answer('cancel')} data-testid="lx-action-sheet-cancel" />
          </view>
        } />
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
