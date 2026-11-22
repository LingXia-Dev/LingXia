import { useLocal } from '@lingxia/view';
import { Said, unused } from '../example';
import { Button } from '../Button/Button';
import { LayerContext } from '../internal/layer';
import { TextArea } from '../TextArea/TextArea';
import { TextInput } from '../TextInput/TextInput';
import { Dialog, type DialogProps } from './Dialog';
import { dialogRecipe } from './Dialog.recipe';

/** A confirmation that interrupts: Escape and back cancel, a backdrop tap does not; the destructive action is last. */
export function Confirm() {
  const open = useLocal('open', false);
  const deleted = useLocal('deleted', false);
  return (
    <view className="gap-2">
      <Button label="Delete note" variant="destructive" width="content" onClick={() => open.set(true)} data-testid="confirm-open" />
      {deleted.value && <text role="status" data-testid="outcome" className="text-footnote text-muted">Deleted</text>}
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
        dismissOn="closeRequest"
        data-testid="rename-dialog"
        actions={<Button label="Save" variant="primary" isDisabled={name.value.trim() === ''} onClick={() => open.set(false)} data-testid="save" />}
      >
        <TextInput bind={name} label="Name" hasAutoFocus enterKeyHint="done" onSubmit={() => open.set(false)} data-testid="name" />
      </Dialog>
    </view>
  );
}

/** A dialog that introduces something: a picture reaches the panel's top edges, with the close button over it. */
export function Hero() {
  const open = useLocal('open', false);
  return (
    <view className="flex-row">
      <Button label="What is new" width="content" onClick={() => open.set(true)} data-testid="hero-open" />
      <Dialog
        isOpen={open.value}
        onOpenChange={next => open.set(next)}
        label="Shared notebooks"
        description="Invite people to a notebook and write in it together. Changes show as they are made."
        size="sm"
        media={<image src="public/photos/photo-2.png" alt="" className="h-40 w-full object-cover" />}
        data-testid="hero-dialog"
        actions={
          <>
            <Button label="Not now" onClick={() => open.set(false)} />
            <Button label="Try it" variant="primary" onClick={() => open.set(false)} data-testid="hero-try" />
          </>
        }
      />
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
      <LayerContext.Provider value="overlay">
        <view className="flex-row justify-end gap-1.5">
          <Button label="Keep" size="sm" onClick={unused} />
          <Button label="Delete" variant="destructive" size="sm" onClick={unused} />
        </view>
      </LayerContext.Provider>
    </view>
  );
}

/**
 * The panel drawn at rest on a scrim, for the gallery's parts stage: Dialog's own part classes,
 * without the overlay, so it sits in the page; its actions sit on the overlay's layer as in a dialog.
 */
export function Parts() {
  return (
    <Said>
      {say => (
        <view className="items-center rounded-container bg-scrim px-4 py-8">
          <view className="w-full max-w-[360px] p-5 gap-4 rounded-dialog border border-text/12 bg-surface-overlay shadow-modal">
            <view className={dialogRecipe.header.base}>
              <text className={dialogRecipe.title.base}>Delete this note?</text>
            </view>
            <text className={dialogRecipe.description.base}>It leaves every device you are signed in on.</text>
            <LayerContext.Provider value="overlay">
              <view className={dialogRecipe.footer.base}>
                <Button label="Cancel" onClick={() => say('Kept the note')} />
                <Button label="Delete" variant="destructive" onClick={() => say('Deleted the note')} />
              </view>
            </LayerContext.Provider>
          </view>
        </view>
      )}
    </Said>
  );
}

/** Full screen: a task with its own content to scroll, closed from its header. */
export function Full() {
  const open = useLocal('open', false);
  const title = useLocal('title', 'Trip to the coast');
  const body = useLocal('body', '');
  return (
    <view className="gap-2">
      <Button label="New note" variant="primary" width="content" onClick={() => open.set(true)} data-testid="full-open" />
      <Dialog
        isOpen={open.value}
        onOpenChange={next => open.set(next)}
        label="New note"
        size="full"
        dismissOn="closeRequest"
        data-testid="full-dialog"
        actions={
          <>
            <Button label="Discard" onClick={() => open.set(false)} data-testid="full-discard" />
            <Button label="Save note" variant="primary" onClick={() => open.set(false)} data-testid="full-save" />
          </>
        }
      >
        <TextInput bind={title} label="Title" />
        <TextArea bind={body} label="Note" rows={8} maxRows={16} />
      </Dialog>
    </view>
  );
}

/** The playground's props that are not plain values. */
export function playground() {
  return {
    actions: <Button label="Delete" variant="destructive" onClick={unused} />,
  } satisfies Partial<DialogProps>;
}
