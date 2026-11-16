import { cx, useBox, type Child, type Local } from '@lingxia/view';
import { IconButton } from '../IconButton/IconButton';
import { composerRecipe as r } from './Composer.recipe';

export type ComposerBinding =
  | {
      /** A View-local cell holding the draft; use it or `value` with `onChange`. */
      bind: Local<string>;
      value?: never;
      onChange?: never;
    }
  | {
      bind?: never;
      /** The draft; pass `onChange` with it. */
      value: string;
      onChange: (value: string) => void;
    };

export type ComposerProps = ComposerBinding & {
  /** The field\'s accessible name: "Message", "Reply to Kai". Not drawn. */
  label: string;
  /** A hint shown while the draft is empty. */
  placeholder?: string;
  /** Runs with the draft, trimmed, from the send button or the keyboard\'s send key. The app sends it and empties the draft. */
  onSubmit: (text: string) => void;
  /** The send button\'s name: "Send". */
  sendLabel: string;
  /** The send button shows a spinner and takes no press; the draft stays editable. @default false */
  isSending?: boolean;
  /** Before the text, or under it at the start once the draft takes more than a line: an IconButton that attaches a file or a picture. */
  startContent?: Child;
  /** What is attached, shown inside the composer over the text: Tokens or Thumbnails, each with its own way to remove it. */
  attachments?: Child;
  /** @default false */
  isDisabled?: boolean;
  /** Why it is disabled, shown under the field: "Only members can reply." */
  disabledMessage?: string;
  className?: string;
  /** On the field; the send button gets `<testid>-send`. */
  'data-testid'?: string;
};

// In points: the send button at the medium size, the gap between two parts, and what the text keeps
// clear at its sides, with a little to spare so the change comes just before the text would wrap.
const sendWidth = 40;
const gap = 4;
const textInset = 24;
// A first line longer than this is never measured: it cannot fit beside the buttons.
const longest = 200;

/** Where a message is written and sent: one surface holding the text, which grows to five lines, and its buttons. */
export function Composer(props: ComposerProps) {
  const { label, placeholder, onSubmit, sendLabel, isSending = false, startContent, attachments, isDisabled = false, disabledMessage, className, 'data-testid': testId } = props;
  const draft = props.bind ? props.bind.value : props.value;
  const text = draft.trim();
  const send = () => { if (text !== '' && !isSending && !isDisabled) onSubmit(text); };
  const surface = useBox('surface');
  const start = useBox('start');
  const line = useBox('line');
  // One row while the draft fits on one line beside the buttons; past that the text takes the full
  // width and the buttons go under it. The test is the draft's own width against the composer's,
  // neither of which the choice changes, so typing at the boundary cannot flip it back and forth.
  const room = surface.value === null ? null
    : surface.value.width - sendWidth - gap - textInset - (startContent !== undefined ? (start.value?.width ?? sendWidth) + gap : 0);
  const isStacked = attachments !== undefined || draft.includes('\n') || draft.length > longest
    || (room !== null && line.value !== null ? line.value.width > room : draft.length > 28);
  const editor = {
    rows: 1,
    maxRows: 5,
    'aria-label': label,
    placeholder,
    enterKeyHint: 'send' as const,
    onSubmit: send,
    disabled: isDisabled,
    'data-testid': testId,
    className: r.editor.base,
  };
  return (
    <view className={cx(r.root.base, className)}>
      <view box={surface} className={cx(r.surface.base, isDisabled && r.surface.disabled)}>
        {attachments !== undefined && <view className={r.attachments.base}>{attachments}</view>}
        {startContent !== undefined && <view box={start} className={cx(r.start.base, isStacked && r.start.stacked)}>{startContent}</view>}
        <view className={cx(r.field.base, isStacked ? r.field.stacked : startContent !== undefined ? r.field.row : r.field.rowAlone)}>
          {props.bind ? <textarea {...editor} bind={props.bind} /> : <textarea {...editor} value={props.value} onInput={props.onChange} />}
        </view>
        <IconButton
          label={sendLabel}
          icon="send"
          variant="primary"
          isDisabled={isDisabled || (text === '' && !isSending)}
          isLoading={isSending}
          onClick={send}
          className={cx(r.send.base, isStacked && r.send.stacked)}
          data-testid={testId && `${testId}-send`}
        />
        {/* The draft's first line as it would be set on one line, measured and never seen. */}
        <view aria-hidden className={r.measure.base}>
          <text box={line} className={r.measure.line}>{draft.split('\n', 1)[0].slice(0, longest)}</text>
        </view>
      </view>
      {isDisabled && disabledMessage !== undefined && <text className={r.reason.base}>{disabledMessage}</text>}
    </view>
  );
}
