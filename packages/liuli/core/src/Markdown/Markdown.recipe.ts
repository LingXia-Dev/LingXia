import type { MarkdownSize } from './Markdown';

export const markdownParts = {
  root: 'view',
  heading: 'text',
  paragraph: 'text',
  run: 'span',
  link: 'link',
  list: 'view',
  item: 'view',
  marker: 'text',
  itemBody: 'view',
  taskSlot: 'view',
  taskGhost: 'text',
  task: 'view',
  quote: 'view',
  code: 'scroll',
  rule: 'view',
  cell: 'text',
} as const;

/**
 * Reading text: paragraphs in the body role with open line height, headings a step apart in
 * weight more than in size, lists hung from quiet markers. Code sits on a tint of the text
 * colour. Nothing is coloured but links.
 */
export const markdownRecipe = {
  root: {
    base: 'self-stretch',
    size: { sm: 'gap-2', md: 'gap-3' } satisfies Record<MarkdownSize, string>,
    selectable: 'select-text',
  },
  heading: {
    base: 'text-text font-semibold tracking-tight',
    level: { 1: 'text-title pt-1', 2: 'text-callout pt-1', 3: 'text-body', 4: 'text-footnote text-muted' } satisfies Record<1 | 2 | 3 | 4, string>,
  },
  paragraph: {
    base: 'text-text',
    size: { sm: 'text-footnote', md: 'text-body' } satisfies Record<MarkdownSize, string>,
  },
  run: {
    strong: 'font-semibold',
    emphasis: 'italic',
    struck: 'line-through text-muted',
    code: 'font-mono bg-text/8',
  },
  link: {
    base: 'text-accent underline active:text-accent/70',
  },
  list: {
    base: 'self-stretch gap-1',
  },
  item: {
    base: 'flex-row items-start gap-2 self-stretch',
  },
  marker: {
    base: 'shrink-0 min-w-5 text-end text-muted tabular-nums',
  },
  itemBody: {
    base: 'grow shrink basis-0 min-w-0 gap-1',
  },
  // The mark is centred on the first line of its item: an empty run in the item's type gives the slot that line's height at any text size.
  taskSlot: {
    base: 'shrink-0 flex-row items-center justify-end',
    // In the item's type, so the column grows with its mark.
    size: { sm: 'text-footnote w-[1.54em]', md: 'text-body w-[1.33em]' } satisfies Record<MarkdownSize, string>,
  },
  taskGhost: {
    base: '',
  },
  task: {
    base: 'shrink-0 items-center justify-center rounded-checkbox border-2',
    // In the item's type, so the mark grows with the text, capped by the View at 1.5×.
    size: { sm: 'text-footnote size-[1.23em]', md: 'text-body size-[1.07em]' } satisfies Record<MarkdownSize, string>,
    open: 'border-line-strong',
    done: 'border-accent-solid bg-accent-solid text-on-accent-solid',
    tick: { sm: 'size-[0.92em]', md: 'size-[0.8em]' } satisfies Record<MarkdownSize, string>,
  },
  quote: {
    base: 'self-stretch gap-2 ps-3 border-s-2 border-line-strong',
  },
  code: {
    base: 'self-stretch overflow-x-auto rounded-control bg-text/6 p-3',
    text: 'self-start font-mono text-footnote text-text whitespace-pre',
  },
  rule: {
    base: 'self-stretch h-px bg-line',
  },
  cell: {
    base: 'text-footnote',
  },
} as const;
