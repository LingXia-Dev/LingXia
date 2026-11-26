import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Drawer',
  category: 'overlays',
  status: 'preview',
  since: '2026-11-15',
  summary: 'A panel that slides in from a side edge',
  description:
    'Drawer is a modal panel the height of the screen on its end or start edge, with the same title, body and footer as a Dialog. ' +
    'It suits a wide screen, where it leaves the page it came from in view beside it: filters for a list, the details of a row, an app\'s places. ' +
    'On a phone it takes most of the width; a BottomSheet is often the better fit there.',
  keywords: ['drawer', 'side sheet', 'side panel', 'slide over', 'off-canvas', 'sidebar'],
  propsType: 'DrawerProps',
  recipe: { map: 'drawerRecipe', parts: 'drawerParts' },
  anatomy: [
    { part: 'panel', required: true, description: 'The dialog primitive on a side edge, rounded on its inner side.' },
    { part: 'title', required: true, description: '`label` as a level-2 heading.' },
    { part: 'close', required: false, description: 'A close button, unless `hasCloseButton` is false; `closeLabel` names it.' },
    { part: 'description', required: false, description: 'Text under the title.' },
    { part: 'body', required: false, description: '`children`, in a scroll that takes the height left. A hairline shows at an edge while content is scrolled under it.' },
    { part: 'headerAction', required: false, description: '`headerAction`: one small control beside the title that acts on the whole panel, such as Refresh.' },
    { part: 'footer', required: false, description: '`actions`: on a phone they share the width equally, the main one last; wider, they keep their own widths at the end, wrapping when narrow.' },
  ],
  accessibility: {
    pattern: 'dialog',
    name: 'label',
    description: ['description'],
    states: [],
    keyboard: [
      { keys: 'Escape', effect: 'Closes it, unless `dismissOn` is `none`' },
      { keys: 'Tab', effect: 'Through the drawer only, while it is open' },
    ],
    target: { md: 32 },
    contrast: [
      { what: 'title', pair: ['text', 'surface-overlay'], minimum: '4.5:1' },
      { what: 'description', pair: ['text-muted', 'surface-overlay'], minimum: '4.5:1' },
    ],
    notes: [
      'A dialog named by its title: assistive tech reads only the drawer while it is open, and focus returns to what opened it.',
      'It mirrors with the reading direction: `end` is the right edge in left-to-right text.',
      'Under reduced motion it fades and does not slide.',
    ],
  },
  guidance: {
    do: [
      'Use `end` for what belongs to the page: filters, details, a short form. Use `start` for navigation.',
      'Give it a close button; a backdrop tap is easy to miss as a way out.',
    ],
    dont: [
      'Use a drawer to confirm something; that is a Dialog.',
      'Open a drawer from a drawer.',
    ],
  },
  examples: [
    { name: 'Filters', title: 'Filters, from the end edge' },
    { name: 'Navigation', title: 'Navigation, from the start edge' },
  ],
  gaps: [],
  limits: [
    'It does not follow the finger; it opens and closes as a whole.',
    'It is always modal: a panel that stays open beside the page is the aside of a Split.',
  ],
  playground: { props: { isOpen: false, label: 'Filters' }, fixture: true, opener: 'Show filters' },
  related: ['Dialog', 'BottomSheet', 'Split'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'A side panel',
    order: 8,
    page: {
      lead: 'Use a drawer for a panel of its own beside the page it came from.',
      hero: 'Filters',
      variants: [{ name: 'Navigation', title: 'From the start edge, narrow, for an app\'s places' }],
    },
  },
};
