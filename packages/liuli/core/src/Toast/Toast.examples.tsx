import { useLocal } from '@lingxia/view';
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
    <ToastRegion dismissLabel="Dismiss">
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
    <ToastRegion dismissLabel="Dismiss">
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
    toast.show({ id: 'upload', message: next < 3 ? `Uploaded ${next} of 3` : 'All 3 photos uploaded', tone: next < 3 ? 'neutral' : 'success' });
  };
  return <Button label="Upload a photo" onClick={upload} isDisabled={done.value >= 3} data-testid="upload" />;
}

/** A progress message updated in place by its id, rather than queued behind itself. */
export function Progress() {
  return (
    <ToastRegion dismissLabel="Dismiss">
      <Uploader />
    </ToastRegion>
  );
}

/**
 * Each face at the width a toast takes on a phone: a message, success, with Undo, and danger with
 * Retry. 32 px apart and below, the raised shadow's reach (8 px down plus a 24 px blur), so each
 * shadow ends before the next card and the stage's edge.
 */
export function Faces() {
  return (
    <view className="gap-8 pb-8">
      <Toast message="Note moved to Travel" onDismiss={() => {}} dismissLabel="Dismiss" />
      <Toast message="Changes saved" tone="success" onDismiss={() => {}} dismissLabel="Dismiss" />
      <Toast message={'"Q3 plan" archived'} action={{ label: 'Undo', onClick: () => {} }} onDismiss={() => {}} dismissLabel="Dismiss" />
      <Toast message="Could not save the note" tone="danger" action={{ label: 'Retry', onClick: () => {} }} onDismiss={() => {}} dismissLabel="Dismiss" />
    </view>
  );
}

/** The home tile. */
export function Specimen() {
  return <Toast message="Changes saved" tone="success" className="w-56" />;
}
