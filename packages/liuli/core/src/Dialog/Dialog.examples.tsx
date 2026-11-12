import { useLocal } from '@lingxia/view';
import { Button } from '../Button/Button';
import { TextInput } from '../TextInput/TextInput';
import { Dialog } from './Dialog';

/** A confirmation that interrupts: Escape and back cancel, a backdrop tap does not; the destructive action is last. */
export function Confirm() {
  const open = useLocal('open', false);
  const deleted = useLocal('deleted', false);
  return (
    <view className="gap-2">
      <Button label="Delete note" variant="destructive" width="content" onClick={() => open.set(true)} data-testid="open" />
      <text role="status" data-testid="outcome" className="text-sm text-muted">{deleted.value ? 'Deleted' : 'Not deleted'}</text>
      <Dialog
        isOpen={open.value}
        onOpenChange={next => open.set(next)}
        label="Delete this note?"
        description="It leaves every device you are signed in on."
        size="sm"
        isAlert
        dismissOn="closeRequest"
        data-testid="dialog"
        actions={
          <>
            <Button label="Cancel" onClick={() => open.set(false)} data-testid="cancel" />
            <Button label="Delete" variant="destructive" onClick={() => { deleted.set(true); open.set(false); }} data-testid="confirm" />
          </>
        }
      />
    </view>
  );
}

/**
 * A short form. The draft lives outside the dialog, since closing discards the dialog's own state.
 * Only Escape and back cancel, so a stray backdrop tap does not lose the typing.
 */
export function Form() {
  const open = useLocal('open', false);
  const name = useLocal('name', '');
  return (
    <view className="gap-2">
      <Button label="Rename" width="content" onClick={() => open.set(true)} data-testid="open" />
      <Dialog
        isOpen={open.value}
        onOpenChange={next => open.set(next)}
        label="Rename notebook"
        closeLabel="Close"
        dismissOn="closeRequest"
        data-testid="dialog"
        actions={<Button label="Save" variant="primary" isDisabled={name.value.trim() === ''} onClick={() => open.set(false)} data-testid="save" />}
      >
        <TextInput bind={name} label="Name" hasAutoFocus enterKeyHint="done" onSubmit={() => open.set(false)} data-testid="name" />
      </Dialog>
    </view>
  );
}
