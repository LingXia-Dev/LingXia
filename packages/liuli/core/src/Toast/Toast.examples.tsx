import { useLocal } from '@lingxia/view';
import { Said } from '../example';
import { Button } from '../Button/Button';
import { Item, ItemList } from '../Item/Item';
import { Toast, ToastRegion, useToast } from './Toast';

const allNotes = ['Q3 plan', 'Reading list', 'Interview notes'];

function Notes() {
  const toast = useToast();
  const archived = useLocal<string[]>('archived', []);
  const shown = allNotes.filter(note => !archived.value.includes(note));
  const archive = (note: string) => {
    archived.set([...archived.value, note]);
    toast.show({
      id: 'archived',
      message: `"${note}" archived`,
      action: { label: 'Undo', onClick: () => archived.set(archived.value.filter(other => other !== note)) },
    });
  };
  return (
    <view className="gap-3">
      {shown.length > 0 ? (
        <ItemList label="Notes">
          {shown.map(note => (
            <Item key={note} label={note} endContent={<Button label="Archive" size="sm" variant="ghost" onClick={() => archive(note)} data-testid={`archive-${note.toLowerCase().replace(/[^a-z0-9]+/g, '-')}`} />} />
          ))}
        </ItemList>
      ) : (
        <text className="text-body text-muted">Every note is archived.</text>
      )}
    </view>
  );
}

/** Archiving a note, with Undo: the toast stays until Undo, dismiss or a swipe closes it. */
export function Archive() {
  return (
    <ToastRegion>
      <Notes />
    </ToastRegion>
  );
}

function ToneButtons() {
  const toast = useToast();
  return (
    <view className="flex-row flex-wrap gap-2">
      <Button label="Saved" size="sm" onClick={() => toast.show({ message: 'Changes saved', tone: 'success' })} data-testid="show-success" />
      <Button label="Shared" size="sm" onClick={() => toast.show({ message: 'Link copied', tone: 'info' })} data-testid="show-info" />
      <Button label="Offline" size="sm" onClick={() => toast.show({ message: 'Offline: changes will sync later', tone: 'warning', duration: 8000 })} data-testid="show-warning" />
      <Button label="Failed" size="sm" onClick={() => toast.show({ message: 'Could not save the note', tone: 'danger' })} data-testid="show-danger" />
    </view>
  );
}

/** One per tone; pressed in a row, they queue and show one at a time. */
export function Tones() {
  return (
    <ToastRegion>
      <ToneButtons />
    </ToastRegion>
  );
}

function Uploader() {
  const toast = useToast();
  const done = useLocal('done', 0);
  const upload = () => {
    const next = Math.min(done.value + 1, 3);
    done.set(next);
    toast.show(next < 3
      ? { id: 'upload', message: `Uploading photos: ${next} of 3 done`, isBusy: true }
      : { id: 'upload', message: 'All 3 photos uploaded', tone: 'success' });
  };
  return <Button label="Upload a photo" onClick={upload} isDisabled={done.value >= 3} data-testid="upload" />;
}

/** A progress message updated in place by its id, rather than queued behind itself: busy, so it stays, until the last update says it is done. */
export function Progress() {
  return (
    <ToastRegion>
      <Uploader />
    </ToastRegion>
  );
}

/**
 * Each face at the width a toast takes on a phone: a message, success, busy, with Undo, and danger
 * with Retry. 32 px apart and below, the raised shadow's reach (8 px down plus a 24 px blur), so each
 * shadow ends before the next card and the stage's edge.
 */
export function Faces() {
  return (
    <Said>
      {say => (
        <view className="gap-8 pb-8">
          <Toast message="Note moved to Travel" onDismiss={() => say('Dismissed')} />
          <Toast message="Changes saved" tone="success" onDismiss={() => say('Dismissed')} />
          <Toast message="Exporting 24 notes" isBusy onDismiss={() => say('Dismissed')} />
          <Toast message={'"Q3 plan" archived'} action={{ label: 'Undo', onClick: () => say('Undid the archive') }} onDismiss={() => say('Dismissed')} />
          <Toast message="Could not save the note" tone="danger" action={{ label: 'Retry', onClick: () => say('Retried') }} onDismiss={() => say('Dismissed')} />
        </view>
      )}
    </Said>
  );
}

/** The home tile. */
export function Specimen() {
  return <Toast message="Saved" tone="success" className="w-full" />;
}
