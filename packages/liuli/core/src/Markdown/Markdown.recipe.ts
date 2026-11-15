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
  // The mark is centred on the first line of its item.
  taskSlot: {
    size: { sm: 'shrink-0 w-5 h-[1.125rem] items-end justify-center', md: 'shrink-0 w-5 h-[1.375rem] items-end justify-center' } satisfies Record<MarkdownSize, string>,
  },
  task: {
    base: 'size-4 items-center justify-center rounded-[4px] border-2',
    open: 'border-line-strong',
    done: 'border-accent bg-accent text-on-accent',
    tick: 'size-3',
  },
  quote: {
    base: 'self-stretch gap-2 ps-3 border-s-2 border-line-strong',
  },
  code: {
    base: 'self-stretch overflow-x-auto rounded-control bg-text/6 p-3',
    text: 'self-start font-mono text-footnote text-text whitespace-pre',
  },
  rule: {
    base: 'self-stretch h-px bg-text/12',
  },
  cell: {
    base: 'text-footnote',
  },
} as const;
