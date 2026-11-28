import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Composer',
  category: 'inputs',
  status: 'preview',
  since: '2026-11-15',
  summary: 'Where a message is written and sent',
  description:
    'Composer is one surface holding the text and its buttons. While the draft fits on one line it is a single row: attach, text, send. Past one line the text takes the full width, growing to five lines and then scrolling, and the buttons sit under it. ' +
    'The send button wakes when there is something to send. ' +
    'The keyboard\'s send key sends too; with a hardware keyboard Shift and Return add a line. The app sends the text and empties the draft. ' +
    'It sits at the bottom of a conversation, where the page keeps it above the keyboard. ' +
    '`endActions` puts the app\'s buttons before send, a voice button or a model picker. `triggers` open a list at the caret when a word starts with their character, `/` for commands or `@` for people: what follows narrows it, the arrow keys move through it and Return or a press puts the suggestion in the draft.',
  keywords: ['composer', 'message input', 'chat input', 'reply', 'send', 'comment box', 'slash command', 'mention', 'prompt'],
  propsType: 'ComposerProps',
  otherPropsTypes: ['ComposerTrigger', 'ComposerSuggestion'],
  recipe: { map: 'composerRecipe', parts: 'composerParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The surface, and under it the reason while disabled.' },
    { part: 'surface', required: true, description: 'The one bordered box; focus turns all of its border to a 2 px accent edge.' },
    { part: 'attachments', required: false, description: 'What is attached, over the text.' },
    { part: 'start', required: false, description: 'A button that attaches: before the text, or under it at the start.' },
    { part: 'field', required: true, description: 'The text\'s place: between the buttons, or the full width over them.' },
    { part: 'editor', required: true, description: 'The text, named by `label`, one to five lines.' },
    { part: 'send', required: true, description: 'An IconButton named by `sendLabel`; disabled while the draft is empty, loading while sending.' },
    { part: 'end', required: false, description: 'The app\'s `endActions` and the send button, together in its place.' },
    { part: 'suggestions', required: false, description: 'A trigger\'s list at the caret, a menu\'s panel; the active suggestion is marked as a hovered item is.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: [],
    states: [
      { state: 'disabled', from: 'isDisabled; the send button also while the draft is empty' },
      { state: 'busy', from: 'isSending, on the send button, unless `onStop` makes it the stop button' },
    ],
    keyboard: [
      { keys: 'Return', effect: 'Sends, with a hardware keyboard' },
      { keys: 'Shift and Return', effect: 'Adds a line' },
      { keys: 'Tab', effect: 'From the field to the end actions and the send button' },
      { keys: 'Up and Down arrows', effect: 'With a trigger\'s list open, move through it' },
      { keys: 'Return', effect: 'With a trigger\'s list open, puts the active suggestion in the draft' },
      { keys: 'Escape', effect: 'Closes a trigger\'s list until the caret leaves the word' },
    ],
    target: { md: 40 },
    contrast: [],
    notes: [
      'The field is named by `label` and the button by `sendLabel`; neither name is drawn.',
      'The send button is unavailable while there is nothing to send, and reports busy while sending.',
      'A draft of only spaces is not sent.',
    ],
  },
  guidance: {
    do: [
      'Empty the draft when the message has been taken, and keep it when sending fails.',
      'Say in `placeholder` who or what it goes to: "Message Kai".',
      'Give `onStop` when what is sent can be stopped, such as a reply being written.',
      'To move focus to this field from code — after a failed submit, say — call `focus()` on the cell it is bound to: `const ok = await actions.send(); if (!ok) draft.focus();`.',
    ],
    dont: [
      'Use it for a long text with a title; that is a form with a TextArea.',
      'Disable it without saying why; give `disabledMessage`.',
    ],
  },
  examples: [
    { name: 'Reply', title: 'Write and send' },
    { name: 'RowWithAttach', title: 'With a way to attach' },
    { name: 'WithAttach', title: 'A draft past one line' },
    { name: 'Attached', title: 'With attachments' },
    { name: 'Stoppable', title: 'A stop button while sending' },
    { name: 'Triggers', title: 'Commands and people at the caret, and a voice button' },
  ],
  gaps: [],
  limits: [
    'It shows what is attached and leaves choosing and uploading files to the app.',
  ],
  playground: { props: { value: '', label: 'Message', placeholder: 'Message Kai', sendLabel: 'Send' }, fixture: true },
  related: ['MessageBubble', 'TextArea', 'IconButton', 'Token'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Write and send',
    order: 14,
    page: {
      lead: 'Use a composer to write and send a message.',
      hero: 'Reply',
      stage: 'bare',
      variants: [
        { name: 'RowWithAttach', title: 'With a way to attach: attach, text and send in one row' },
        { name: 'WithAttach', title: 'Past one line the text takes the full width, to five lines; the buttons go under it' },
        { name: 'Attached', title: 'What is attached sits inside, over the text' },
        { name: 'StateSending', title: 'Sending: the button is busy, the draft stays' },
        { name: 'Stoppable', title: 'Sending what can be stopped: the button is Stop' },
        { name: 'StateDisabled', title: 'Disabled, with the reason' },
        { name: 'Triggers', title: 'An assistant\'s composer: / for commands and @ for people at the caret, a voice button before send' },
      ],
    },
  },
};
