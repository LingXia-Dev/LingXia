import { cx, type Child, type Local } from '@lingxia/view';
import { IconButton } from '../IconButton/IconButton';
import { TextArea } from '../TextArea/TextArea';
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
  /** Before the field: an IconButton that attaches a file or a picture. */
  startContent?: Child;
  /** @default false */
  isDisabled?: boolean;
  /** Why it is disabled, shown under the field: "Only members can reply." */
  disabledMessage?: string;
  className?: string;
  /** On the field; the send button gets `<testid>-send`. */
  'data-testid'?: string;
};

/** Where a message is written and sent: a field that grows to five lines, and a send button. */
export function Composer(props: ComposerProps) {
  const { label, placeholder, onSubmit, sendLabel, isSending = false, startContent, isDisabled = false, disabledMessage, className, 'data-testid': testId } = props;
  const draft = props.bind ? props.bind.value : props.value;
  const text = draft.trim();
  const send = () => { if (text !== '' && !isSending && !isDisabled) onSubmit(text); };
  const field = {
    label,
    isLabelHidden: true,
    placeholder,
    rows: 1,
    maxRows: 5,
    enterKeyHint: 'send' as const,
    onSubmit: send,
    isDisabled,
    className: r.field.base,
    'data-testid': testId,
  };
  return (
    <view className={cx(r.root.base, className)}>
      <view className={r.row.base}>
      {startContent !== undefined && <view className={r.start.base}>{startContent}</view>}
      {props.bind ? <TextArea {...field} bind={props.bind} /> : <TextArea {...field} value={props.value} onChange={props.onChange} />}
      <IconButton
        label={sendLabel}
        icon="send"
        variant="primary"
        isDisabled={isDisabled || (text === '' && !isSending)}
        isLoading={isSending}
        onClick={send}
        className={r.send.base}
        data-testid={testId && `${testId}-send`}
      />
      </view>
      {isDisabled && disabledMessage !== undefined && <text className={r.reason.base}>{disabledMessage}</text>}
    </view>
  );
}
