import { useLocal } from '@lingxia/view';
import { TextArea } from './TextArea';

/** Bound to a View-local cell; it grows with the text up to six lines, then scrolls. */
export function Bound() {
  const note = useLocal('note', '');
  return (
    <TextArea
      bind={note}
      label="Delivery note"
      placeholder="Gate code, where to leave it"
      description="The courier sees this when they arrive."
      rows={3}
      maxRows={6}
      maxLength={280}
      data-testid="note"
    />
  );
}

/** A reply box: Send on the keyboard submits, Shift+Return adds a line. */
export function Reply() {
  const draft = useLocal('draft', '');
  const sent = useLocal('sent', '');
  return (
    <view className="gap-2">
      <TextArea
        value={draft.value}
        onChange={next => draft.set(next)}
        label="Reply"
        isLabelHidden
        placeholder="Write a reply"
        rows={1}
        maxRows={5}
        enterKeyHint="send"
        onSubmit={text => { sent.set(text); draft.set(''); }}
        data-testid="reply"
      />
      {sent.value !== '' && <text data-testid="sent" className="text-footnote text-muted">{sent.value}</text>}
    </view>
  );
}

/** A required field with an error that names the fix. */
export function Invalid() {
  const reason = useLocal('reason', '');
  const short = reason.value.trim().length < 10;
  return (
    <TextArea
      bind={reason}
      label="Why are you leaving?"
      isRequired
      status={short ? { type: 'error', message: 'Write at least 10 characters.' } : undefined}
      data-testid="reason"
    />
  );
}

/** The home tile. */
export function Specimen() {
  return <TextArea value="Leave it by the blue door." onChange={() => {}} label="Note" isLabelHidden size="sm" rows={2} className="w-32" />;
}

export function Sizes() {
  const text = useLocal('text', '');
  return (
    <view className="gap-3">
      <TextArea bind={text} label="Small" size="sm" rows={2} placeholder="A short note" />
      <TextArea bind={text} label="Medium" rows={2} placeholder="A short note" />
      <TextArea bind={text} label="Large" size="lg" rows={2} placeholder="A short note" />
    </view>
  );
}

export function StateEmpty() {
  return <TextArea value="" onChange={() => {}} label="Comment" placeholder="Add a comment" rows={2} />;
}

export function StateFilled() {
  return <TextArea value={'Lovely place.\nWould stay again.'} onChange={() => {}} label="Comment" rows={2} />;
}

export function StateInvalid() {
  return <TextArea value="Meh" onChange={() => {}} label="Feedback" rows={2} status={{ type: 'error', message: 'Write at least 10 characters.' }} />;
}

export function StateDisabled() {
  return <TextArea value="Sent on 3 May." onChange={() => {}} label="Message" rows={2} isDisabled />;
}

/** A limit the writer can see coming. */
export function Counted() {
  const bio = useLocal('bio', 'Designer in Lisbon. I write about type, maps and small tools.');
  return <TextArea bind={bio} label="About you" description="Shown on your profile." maxLength={160} hasCount data-testid="bio" />;
}
