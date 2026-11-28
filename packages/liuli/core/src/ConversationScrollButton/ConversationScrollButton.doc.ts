import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'ConversationScrollButton',
  category: 'content',
  status: 'preview',
  since: '2026-11-27',
  summary: 'Back to the newest message',
  description:
    '`useConversationScroll(count)` keeps a conversation\'s List on its newest message as messages arrive while the reader is there, and holds it still while they read further up. ' +
    'ConversationScrollButton shows only while they are up: a round button at the end corner over the bottom of the conversation that goes back to the newest message, ' +
    'with the count of messages that arrived meanwhile on its corner. Pass the hook\'s `anchor` and `onScrollEdge` to the List, and put the button after the List in the box that holds both.',
  keywords: ['scroll to bottom', 'jump to latest', 'new messages', 'conversation', 'chat', 'follow', 'streaming', 'unread'],
  propsType: 'ConversationScrollButtonProps',
  otherPropsTypes: ['ConversationScroll'],
  recipe: { map: 'conversationScrollButtonRecipe', parts: 'conversationScrollButtonParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The round button on an opaque raised fill with the floating shadow, over the conversation at its end corner.' },
    { part: 'count', required: false, description: 'How many messages arrived out of view, on the button\'s corner; 99+ past 99.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: [],
    states: [],
    keyboard: [{ keys: 'Enter or Space', effect: 'Scrolls to the newest message' }],
    target: { md: 40 },
    contrast: [
      { what: 'arrow', pair: ['text', 'surface-raised'], minimum: '4.5:1' },
      { what: 'count', pair: ['on-accent', 'accent'], minimum: '4.5:1' },
    ],
    notes: [
      'Its name says how many messages are new, so the count is not only seen.',
      'It leaves the tree at the newest message, so it is never a stop that does nothing.',
    ],
  },
  guidance: {
    do: [
      'Use the hook with every conversation that grows while it is read, a streamed answer included.',
    ],
    dont: [
      'Scroll the reader down when a message arrives while they read further up; the count tells them.',
    ],
  },
  examples: [
    { name: 'Follow', title: 'A conversation that follows new messages, with the button while reading up' },
  ],
  gaps: [],
  limits: [
    'It counts messages, not unread ones; a list with read state counts those itself and passes `label`.',
  ],
  related: ['MessageBubble', 'Composer'],
  gallery: {
    specimen: 'Follow',
    tagline: 'Back to the latest',
    order: 23,
    page: {
      lead: 'Use it with a conversation that grows while it is read.',
      hero: 'Follow',
      variants: [],
    },
  },
};
