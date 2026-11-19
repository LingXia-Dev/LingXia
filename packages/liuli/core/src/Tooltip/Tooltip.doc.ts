import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Tooltip',
  category: 'overlays',
  status: 'preview',
  since: '2026-11-18',
  summary: 'A short hint naming its trigger, shown on hover, focus or a long press',
  description:
    'Tooltip wraps a trigger, usually an IconButton, and shows its name in words while a pointer rests on it or keyboard focus is on it. ' +
    'On a phone it shows on a long press and hides 1.5 s after release, never on a tap, so the tap still presses the trigger. For a note someone opens on purpose, use InfoTip.',
  keywords: ['tooltip', 'hint', 'hover', 'label', 'icon button', 'title'],
  propsType: 'TooltipProps',
  recipe: { map: 'tooltipRecipe', parts: 'tooltipParts' },
  anatomy: [
    { part: 'bubble', required: true, description: 'The hint: a hint popover beside the trigger, in the inverse of the page.' },
    { part: 'text', required: true, description: 'The hint\'s words; the trigger is described by them while shown.' },
  ],
  accessibility: {
    pattern: null,
    name: null,
    description: ['the hint, while shown'],
    states: [],
    keyboard: [
      { keys: 'Tab', effect: 'Focus on the trigger shows the hint; leaving it hides it.' },
      { keys: 'Escape', effect: 'Hides the hint; focus stays on the trigger.' },
    ],
    contrast: [
      { what: 'hint text', pair: ['surface', 'text'], minimum: '4.5:1' },
    ],
    notes: [
      'The trigger keeps its own name; the hint repeats it in words and never says what that name lacks.',
      'A Button, IconButton or ToggleButton takes the hint itself; any other element is wrapped in one plain button, one focus stop.',
    ],
  },
  guidance: {
    do: [
      'Use a `Link` as the trigger for words in a sentence: put the overlay in the `<text>`, around the Link, and the panel opens from the linked words.',
      'Give every icon-only button in a toolbar a tooltip with its name.',
      'Keep it to a few words: "Undo", "Share".',
      'On a phone a tooltip does not show: what someone needs to read there goes in an `InfoTip`, which opens on a tap.',
    ],
    dont: [
      'Put anything in a tooltip that is not already the trigger\'s name; it is not read on a tap, and many never see it.',
      'Put a tooltip on a button whose label is already drawn.',
    ],
  },
  examples: [
    { name: 'Toolbar', title: 'On icon buttons' },
    { name: 'Below', title: 'Opening below' },
    { name: 'OnWords', title: 'On words' },
    { name: 'InSentence', title: 'On a link in a sentence' },
  ],
  gaps: [
    {
      missing: 'Showing on a long press on a phone and hiding 1.5 s after release, never on a tap; where a ContextMenu shares the trigger, the long press opens the menu instead.',
      needs: 'a hint popover that interest shows on a long press on touch',
      today: 'Shown by the View\'s interest: a pointer resting or keyboard focus.',
    },
  ],
  related: ['InfoTip', 'IconButton', 'HoverCard'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'A hint on hover',
    order: 3,
    page: {
      lead: 'Use a tooltip to name an icon button in words. Rest a pointer on the button, or long-press it on a phone.',
      hero: 'Toolbar',
      parts: { name: 'Parts', title: 'The hint' },
      variants: [{ name: 'Below', title: 'Opening below, where there is no room above' }],
      inContext: [{ name: 'OnWords', title: 'On words: wrapped in one plain button' }, { name: 'InSentence', title: 'On a link in a sentence' }],
    },
  },
};
