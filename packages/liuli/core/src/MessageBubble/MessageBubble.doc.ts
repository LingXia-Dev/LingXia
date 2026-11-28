import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'MessageBubble',
  category: 'content',
  status: 'preview',
  since: '2026-11-15',
  summary: 'One message of a conversation',
  description:
    'MessageBubble draws one message: the user\'s own at the end edge in the accent\'s soft step, anyone else\'s at the start on a quiet tint, with a name and a face over the first of a run. ' +
    'Under it goes a line the app writes, the time or how the message stands, and for one that failed a way to send it again. ' +
    'A conversation is the View\'s list of these with a Composer under it; the Conversation pattern shows them together. ' +
    'MessageReasoning folds how an assistant got to its answer into one line above the message, ' +
    'and MessageToolCall each step it took with a tool: a line with where the run stands, its details behind it, and Allow and Deny while the step waits on the user.',
  keywords: ['message', 'bubble', 'chat', 'conversation', 'comment', 'reply', 'reasoning', 'thinking', 'assistant', 'tool', 'tool call', 'step', 'approval', 'agent'],
  propsType: 'MessageBubbleProps',
  otherPropsTypes: ['MessageReasoningProps', 'MessageToolCallProps'],
  recipe: { map: 'messageBubbleRecipe', parts: 'messageBubbleParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The row: it puts the bubble at its side.' },
    { part: 'line', required: true, description: 'The avatar and the bubble, the avatar level with the bubble\'s last line.' },
    { part: 'avatar', required: false, description: 'The writer\'s Avatar, or its width kept empty down a run.' },
    { part: 'author', required: false, description: 'Who wrote it, over the first bubble of a run.' },
    { part: 'bubble', required: true, description: 'The message: up to four fifths of the row, 520 px at most.' },
    { part: 'meta', required: false, description: 'The line under the bubble: time, state, and the retry action.' },
    { part: 'reasoningToggle', required: false, description: 'MessageReasoning\'s one line: a button named by `label`, with a spinner while `isStreaming`.' },
    { part: 'reasoningText', required: false, description: 'The reasoning, behind a rule at the start; it can be selected.' },
    { part: 'toolLine', required: false, description: 'MessageToolCall\'s line: the mark for where it stands, the label, "Failed" or "Denied" in words, the meta; a button when it has details.' },
    { part: 'toolDetails', required: false, description: 'Its details, behind the same rule as the reasoning; open while the step waits.' },
    { part: 'toolAnswers', required: false, description: 'Deny, then Allow, while it waits.' },
  ],
  accessibility: {
    pattern: null,
    name: null,
    description: [],
    states: [
      { state: 'expanded', from: 'MessageReasoning: isOpen, or isDefaultOpen and the presses since' },
      { state: 'busy', from: 'MessageReasoning: isStreaming; MessageToolCall: status running' },
      { state: 'expanded', from: 'MessageToolCall with details: isOpen, or isDefaultOpen and the presses since' },
    ],
    keyboard: [{ keys: 'Enter or Space', effect: 'Opens and closes a MessageReasoning, or a MessageToolCall that has details' }],
    target: {},
    contrast: [
      { what: 'my message', pair: ['on-accent-soft', 'accent-soft'], minimum: '4.5:1' },
      { what: 'their message', pair: ['text', 'text/6'], over: 'canvas', minimum: '4.5:1' },
      { what: 'meta', pair: ['text-muted', 'canvas'], minimum: '4.5:1' },
      { what: 'reasoning, its line and its text', pair: ['text-muted', 'canvas'], minimum: '4.5:1' },
      { what: 'failure words', pair: ['danger', 'canvas'], minimum: '4.5:1' },
      { what: 'a waiting tool step\'s mark', pair: ['accent', 'canvas'], minimum: '3:1' },
      { what: 'a waiting tool step\'s label', pair: ['text', 'canvas'], minimum: '4.5:1' },
    ],
    notes: [
      'It is read in order: the name, the message, then the line under it. Put the list in the View\'s list so a screen reader steps message by message.',
      'A failed message says so in words and with an error mark under the bubble, not with colour alone.',
      'The side tells whose it is to the eye; say it in `author` for the ear, or name the list by who is speaking.',
      'A tool step is named by its label and where it stands ("npm test, failed"), so the mark is never the only sign.',
    ],
  },
  guidance: {
    do: [
      'Write `meta` in full: "Not sent", "Sending", a time.',
      'Join the bubbles of one person\'s run and name the run once.',
      'Label a tool step by what it does, in the user\'s words: "Searched the web for tide times", not the tool\'s name.',
      'Name what allowing does when it cannot be undone: `allowLabel` "Delete 3 files" with `isDestructive`.',
    ],
    dont: [
      'Colour bubbles per person; side and name tell them apart.',
      'Put a whole form in a bubble; a bubble holds a message, a picture or a file.',
    ],
  },
  examples: [
    { name: 'Exchange', title: 'A short exchange' },
    { name: 'States', title: 'Sending, sent and not sent' },
    { name: 'Reasoning', title: 'An assistant\'s reasoning, folded above its answer' },
    { name: 'ToolCalls', title: 'An assistant\'s tool steps: done, failed, running, and one waiting to be allowed' },
  ],
  gaps: [],
  limits: [
    'No reactions, threads or read receipts.',
    'No typing mark.',
  ],
  playground: { props: { text: 'See you at six?', side: 'start', author: 'Ana Lima' } },
  related: ['Composer', 'Avatar', 'Markdown'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'A message',
    order: 16,
    page: {
      lead: 'Use a message bubble for each message of a conversation.',
      hero: 'Exchange',
      variants: [
        { name: 'States', title: 'On its way, sent, and not sent with a way to try again' },
        { name: 'Reasoning', title: 'MessageReasoning: how an assistant got to its answer, one line until opened; a spinner while it is still thinking' },
        { name: 'ToolCalls', title: 'MessageToolCall: each step with a tool on one line, its details behind it; a step that waits asks to be allowed' },
      ],
    },
  },
};
