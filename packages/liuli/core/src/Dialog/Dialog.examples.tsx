import { useLocal } from '@lingxia/view';
import { Button } from '../Button/Button';
import { TextInput } from '../TextInput/TextInput';
import { Dialog } from './Dialog';
import { dialogRecipe } from './Dialog.recipe';

/** A confirmation that interrupts: Escape and back cancel, a backdrop tap does not; the destructive action is last. */
export function Confirm() {
  const open = useLocal('open', false);
  const deleted = useLocal('deleted', false);
  return (
    <view className="gap-2">
      <Button label="Delete note" variant="destructive" width="content" onClick={() => open.set(true)} data-testid="confirm-open" />
      <text role="status" data-testid="outcome" className="text-footnote text-muted">{deleted.value ? 'Deleted' : 'Not deleted'}</text>
      <Dialog
        isOpen={open.value}
        onOpenChange={next => open.set(next)}
        label="Delete this note?"
        description="It leaves every device you are signed in on."
        size="sm"
        isAlert
        dismissOn="closeRequest"
        data-testid="confirm-dialog"
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
      <Button label="Rename" width="content" onClick={() => open.set(true)} data-testid="rename-open" />
      <Dialog
        isOpen={open.value}
        onOpenChange={next => open.set(next)}
        label="Rename notebook"
        closeLabel="Close"
        dismissOn="closeRequest"
        data-testid="rename-dialog"
        actions={<Button label="Save" variant="primary" isDisabled={name.value.trim() === ''} onClick={() => open.set(false)} data-testid="save" />}
      >
        <TextInput bind={name} label="Name" hasAutoFocus enterKeyHint="done" onSubmit={() => open.set(false)} data-testid="name" />
      </Dialog>
    </view>
  );
}

/**
 * The tile: the panel at rest with real buttons, since a real dialog covers the page. shadow-modal's
 * two layers are scaled down so the shadow stays inside the stage.
 */
export function Specimen() {
  return (
    <view className="p-2.5 gap-2 rounded-container border border-text/12 bg-surface-overlay shadow-[0_2px_4px_rgb(0_0_0/0.10),0_6px_14px_rgb(0_0_0/0.18)]">
      <text className="text-footnote font-semibold text-text">Delete note?</text>
      <view className="flex-row justify-end gap-1.5">
        <Button label="Keep" size="sm" />
        <Button label="Delete" variant="destructive" size="sm" />
      </view>
    </view>
  );
}

/**
 * The panel drawn at rest on a scrim, for the gallery's parts stage: Dialog's own part classes,
 * without the overlay, so it sits in the page.
 */
export function Parts() {
  return (
    <view className="items-center rounded-container bg-scrim px-4 py-8">
      <view className="w-full max-w-[360px] p-5 gap-4 rounded-dialog border border-text/12 bg-surface-overlay shadow-modal">
        <view className={dialogRecipe.header.base}>
          <text className={dialogRecipe.title.base}>Delete this note?</text>
        </view>
        <text className={dialogRecipe.description.base}>It leaves every device you are signed in on.</text>
        <view className={dialogRecipe.footer.base}>
          <Button label="Cancel" />
          <Button label="Delete" variant="destructive" />
        </view>
      </view>
    </view>
  );
}
