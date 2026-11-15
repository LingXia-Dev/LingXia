import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Composer',
  category: 'inputs',
  status: 'preview',
  since: '2026-11-15',
  summary: 'Where a message is written and sent',
  description:
    'Composer is a field that starts one line tall and grows with the draft to five lines, then scrolls, beside a send button that wakes when there is something to send. ' +
    'The keyboard\'s send key sends too; with a hardware keyboard Shift and Return add a line. The app sends the text and empties the draft. ' +
    'It sits at the bottom of a conversation, where the page keeps it above the keyboard.',
  keywords: ['composer', 'message input', 'chat input', 'reply', 'send', 'comment box'],
  propsType: 'ComposerProps',
  recipe: { map: 'composerRecipe', parts: 'composerParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The row, and under it the reason while disabled.' },
    { part: 'row', required: true, description: 'What sits beside the field stays on its last line.' },
    { part: 'start', required: false, description: 'Before the field: a button that attaches.' },
    { part: 'field', required: true, description: 'A TextArea named by `label`, one to five lines.' },
    { part: 'send', required: true, description: 'An IconButton named by `sendLabel`; disabled while the draft is empty, loading while sending.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: [],
    states: [
      { state: 'disabled', from: 'isDisabled; the send button also while the draft is empty' },
      { state: 'busy', from: 'isSending, on the send button' },
    ],
    keyboard: [
      { keys: 'Return', effect: 'Sends, with a hardware keyboard' },
      { keys: 'Shift and Return', effect: 'Adds a line' },
      { keys: 'Tab', effect: 'From the field to the send button' },
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
    ],
    dont: [
      'Use it for a long text with a title; that is a form with a TextArea.',
      'Disable it without saying why; give `disabledMessage`.',
    ],
  },
  examples: [
    { name: 'Reply', title: 'Write and send' },
    { name: 'WithAttach', title: 'With a way to attach, and a long draft' },
  ],
  gaps: [],
  limits: [
    'It does not hold attachments itself; show them over it with Thumbnail or FileInput rows.',
  ],
  related: ['MessageBubble', 'TextArea', 'IconButton'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Write and send',
    order: 14,
    page: {
      lead: 'Use a composer to write and send a message.',
      hero: 'Reply',
      variants: [
        { name: 'WithAttach', title: 'A long draft grows the field to five lines; the buttons stay on the last line' },
        { name: 'StateSending', title: 'Sending: the button is busy, the draft stays' },
        { name: 'StateDisabled', title: 'Disabled, with the reason' },
      ],
    },
  },
};
