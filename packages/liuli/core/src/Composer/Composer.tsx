import { cx, useBox, useId, useLocal, type Child, type LocalText } from '@lingxia/view';
import { liuliText } from '../i18n/text';
import { IconButton } from '../IconButton/IconButton';
import { composerRecipe as r } from './Composer.recipe';
import { dataOf } from '../internal/data';
import { LayerContext } from '../internal/layer';

export type ComposerBinding =
  | {
      /** A text cell (`useLocal('')`) holding the draft; its `focus()` focuses the field. Use it or `value` with `onChange`. */
      bind: LocalText;
      value?: never;
      onChange?: never;
    }
  | {
      bind?: never;
      /** The draft; pass `onChange` with it. */
      value: string;
      onChange: (value: string) => void;
    };

export type ComposerStop =
  | { onStop?: never; stopLabel?: never }
  | {
      /** While sending, the send button becomes a stop button that runs this, as when a reply is being written. */
      onStop: () => void;
      /** The stop button's name: "Stop". Defaults to "Stop" in the page's language. */
      stopLabel?: string;
    };

export interface ComposerSuggestion {
  /** Tells it from the others. */
  value: string;
  /** What the list shows and what is matched against what follows the trigger: "summarize", "Ana Lima". */
  label: string;
  /** A quieter line beside the label: what a command does, a person's role. */
  description?: string;
  /** What replaces the trigger and the typed text. @default the trigger, the label and a space: "@Ana Lima " */
  insert?: string;
}

export interface ComposerTrigger {
  /** The character that opens the list at the start of a word: `/` for commands, `@` for people. */
  char: string;
  /** Names the list for assistive tech: "Commands", "People". */
  label: string;
  /** The suggestions, matched by label against what was typed after the trigger; or a function of that text, for a list the app narrows itself. */
  items: readonly ComposerSuggestion[] | ((query: string) => readonly ComposerSuggestion[]);
  /** Runs after a suggestion goes into the draft. */
  onChoose?: (suggestion: ComposerSuggestion) => void;
}

export type ComposerProps = ComposerBinding & ComposerStop & {
  /** The field\'s accessible name: "Message", "Reply to Kai". Not drawn. */
  label: string;
  /** A hint shown while the draft is empty. */
  placeholder?: string;
  /** Runs with the draft, trimmed, from the send button or the keyboard\'s send key. The app sends it and empties the draft. */
  onSubmit: (text: string) => void;
  /** The send button\'s name: "Send". Defaults to "Send" in the page's language. */
  sendLabel?: string;
  /** The send button shows a spinner and takes no press, or becomes the stop button with `onStop`; the draft stays editable. @default false */
  isSending?: boolean;
  /** Before the text, or under it at the start once the draft takes more than a line: an IconButton that attaches a file or a picture. */
  startContent?: Child;
  /** Before the send button: IconButtons for the voice, a model or the mode to send in. */
  endActions?: Child;
  /** Lists that open at the caret when a word starts with their character, `/` or `@`: the arrow keys move through one, Return or a press puts the suggestion in the draft. */
  triggers?: readonly ComposerTrigger[];
  /** What is attached, shown inside the composer over the text: Tokens or Thumbnails, each with its own way to remove it. */
  attachments?: Child;
  /** @default false */
  isDisabled?: boolean;
  /** Why it is disabled, shown under the field: "Only members can reply." */
  disabledMessage?: string;
  className?: string;
  /** On the field; the send button gets `<testid>-send`, a trigger's list `<testid>-suggestions`, a suggestion `<testid>-suggestion-<value>`. */
  'data-testid'?: string;
};

// In points: the send button at the medium size, the gap between two parts, and what the text keeps
// clear at its sides, with a little to spare so the change comes just before the text would wrap.
const sendWidth = 40;
const gap = 4;
const textInset = 24;
// A first line longer than this is never measured: it cannot fit beside the buttons.
const longest = 200;
// Suggestions shown at once; typing narrows the rest.
const mostSuggestions = 8;

/** The trigger a caret sits in: its character starts a word and no space comes between it and the caret. */
function triggerAt(draft: string, caret: number, triggers: readonly ComposerTrigger[]): { trigger: ComposerTrigger; start: number; query: string } | null {
  for (let index = caret - 1; index >= 0; index--) {
    const char = draft[index];
    if (/\s/.test(char)) return null;
    const trigger = triggers.find(each => each.char === char);
    if (trigger && (index === 0 || /\s/.test(draft[index - 1]))) return { trigger, start: index, query: draft.slice(index + 1, caret) };
  }
  return null;
}

function suggestionsFor(trigger: ComposerTrigger, query: string): readonly ComposerSuggestion[] {
  if (typeof trigger.items === 'function') return trigger.items(query).slice(0, mostSuggestions);
  const typed = query.toLowerCase();
  const starts = trigger.items.filter(item => item.label.toLowerCase().startsWith(typed));
  const within = trigger.items.filter(item => !item.label.toLowerCase().startsWith(typed) && item.label.toLowerCase().includes(typed));
  return [...starts, ...within].slice(0, mostSuggestions);
}

/** Where a message is written and sent: one surface holding the text, which grows to five lines, and its buttons. */
export function Composer(props: ComposerProps) {
  const { label, placeholder, onSubmit, sendLabel = liuliText('@liuli.send'), isSending = false, onStop, stopLabel = liuliText('@liuli.stop'), startContent, endActions, triggers, attachments, isDisabled = false, disabledMessage, className, 'data-testid': testId } = props;
  const draft = props.bind ? props.bind.value : props.value;
  const text = draft.trim();
  const send = () => { if (text !== '' && !isSending && !isDisabled) onSubmit(text); };
  const isStoppable = isSending && onStop !== undefined && !isDisabled;
  const surface = useBox('surface');
  const start = useBox('start');
  const end = useBox('end');
  const line = useBox('line');
  const id = useId();
  const [editorId, panelId, listId] = [`${id}-editor`, `${id}-panel`, `${id}-suggestions`];
  const caret = useLocal<number | null>(null);
  // The trigger the person closed with Escape, by where it starts, so it stays closed until they leave it.
  const dismissed = useLocal<number | null>(null);
  const active = useLocal(0);
  const found = triggers !== undefined && caret.value !== null && !isDisabled ? triggerAt(draft, Math.min(caret.value, draft.length), triggers) : null;
  const suggestions = found !== null && found.start !== dismissed.value ? suggestionsFor(found.trigger, found.query) : [];
  const isSuggesting = found !== null && suggestions.length > 0;
  const at = Math.min(active.value, suggestions.length - 1);
  const choose = (suggestion: ComposerSuggestion) => {
    if (found === null) return;
    const to = Math.min(caret.value ?? draft.length, draft.length);
    const text = suggestion.insert ?? `${found.trigger.char}${suggestion.label} `;
    if (props.bind) props.bind.replaceRange(found.start, to, text);
    else props.onChange(draft.slice(0, found.start) + text + draft.slice(to));
    caret.set(found.start + text.length);
    active.set(0);
    found.trigger.onChoose?.(suggestion);
  };
  const keys = isSuggesting ? {
    ArrowDown: () => active.set((at + 1) % suggestions.length),
    ArrowUp: () => active.set((at - 1 + suggestions.length) % suggestions.length),
    Enter: () => choose(suggestions[at]),
    Escape: () => dismissed.set(found.start),
  } : undefined;
  // One row while the draft fits on one line beside the buttons; past that the text takes the full
  // width and the buttons go under it. The test is the draft's own width against the composer's,
  // neither of which the choice changes, so typing at the boundary cannot flip it back and forth.
  const room = surface.value === null ? null
    : surface.value.width - (endActions !== undefined ? end.value?.width ?? sendWidth * 2 : sendWidth) - gap - textInset - (startContent !== undefined ? (start.value?.width ?? sendWidth) + gap : 0);
  const isStacked = attachments !== undefined || draft.includes('\n') || draft.length > longest
    || (room !== null && line.value !== null ? line.value.width > room : draft.length > 28);
  const editor = {
    id: editorId,
    rows: 1,
    maxRows: 5,
    'aria-label': label,
    placeholder,
    enterKeyHint: 'send' as const,
    onSubmit: send,
    disabled: isDisabled,
    ...(triggers !== undefined ? {
      onSelectionChange: (selection: { start: number; end: number }) => {
        caret.set(selection.start === selection.end ? selection.start : null);
        if (dismissed.value !== null && (selection.start <= dismissed.value || selection.start !== selection.end)) dismissed.set(null);
        active.set(0);
      },
      onKeyDown: keys,
      // The editor keeps focus; a screen reader hears the suggestion the arrows reach.
      ...(isSuggesting ? { 'aria-controls': listId, 'aria-activedescendant': `${listId}-${at}` } : {}),
    } : {}),
    'data-testid': testId, ...dataOf(props),
    className: r.editor.base,
  };
  const sendButton = (classes: string) => (
    <IconButton
      label={isStoppable ? stopLabel : sendLabel}
      icon={isStoppable ? 'stop' : 'send'}
      variant="primary"
      isDisabled={isDisabled || (text === '' && !isSending)}
      isLoading={isSending && !isStoppable}
      onClick={isStoppable ? onStop : send}
      className={classes}
      data-testid={testId && `${testId}-send`}
    />
  );
  return (
    <view className={cx(r.root.base, className)}>
      <view box={surface} className={cx(r.surface.base, isDisabled && r.surface.disabled)}>
        {attachments !== undefined && <view className={r.attachments.base}>{attachments}</view>}
        {startContent !== undefined && <view box={start} className={cx(r.start.base, isStacked && r.start.stacked)}>{startContent}</view>}
        <view className={cx(r.field.base, isStacked ? r.field.stacked : startContent !== undefined ? r.field.row : r.field.rowAlone)}>
          {props.bind ? <textarea {...editor} bind={props.bind} /> : <textarea {...editor} value={props.value} onInput={props.onChange} />}
        </view>
        {endActions === undefined ? sendButton(cx(r.send.base, r.send.alone, isStacked && r.send.stacked)) : (
          <view box={end} className={cx(r.end.base, isStacked && r.end.stacked)}>
            {endActions}
            {sendButton(r.send.base)}
          </view>
        )}
        {/* The draft's first line as it would be set on one line, measured and never seen. */}
        <view aria-hidden className={r.measure.base}>
          <text box={line} className={r.measure.line}>{draft.split('\n', 1)[0].slice(0, longest)}</text>
        </view>
      </view>
      {isDisabled && disabledMessage !== undefined && <text className={r.reason.base}>{disabledMessage}</text>}
      {triggers !== undefined && (
        // A manual popover at the caret takes no focus, so the text keeps the keys while the list is open.
        <view id={panelId} popover="manual" anchor={`${editorId}:caret`} open={isSuggesting} onToggle={open => { if (!open && found !== null) dismissed.set(found.start); }} data-testid={testId && `${testId}-suggestions`} className={r.suggestions.base}>
          <LayerContext.Provider value="overlay">
            <view id={listId} role="listbox" aria-label={found?.trigger.label} className={r.suggestionList.base}>
              {suggestions.map((suggestion, index) => (
                <button
                  key={suggestion.value}
                  id={`${listId}-${index}`}
                  role="option"
                  aria-selected={index === at}
                  onClick={() => choose(suggestion)}
                  data-testid={testId && `${testId}-suggestion-${suggestion.value}`}
                  className={cx(r.suggestion.base, index === at && r.suggestion.active)}
                >
                  <text className={r.suggestionLabel.base}>{suggestion.label}</text>
                  {suggestion.description !== undefined && <text className={r.suggestionDescription.base}>{suggestion.description}</text>}
                </button>
              ))}
            </view>
          </LayerContext.Provider>
        </view>
      )}
    </view>
  );
}
