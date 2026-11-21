export const codeParts = {
  inline: 'span',
  block: 'view',
  header: 'view',
  title: 'text',
  scroll: 'scroll',
  lines: 'view',
  tints: 'view',
  tint: 'view',
  gutter: 'text',
  signs: 'text',
  mark: 'span',
  headerLine: 'span',
  text: 'text',
} as const;

/**
 * Code is the one text whose meaning needs a face: the platform's fixed-width one, by its generic
 * name, on a tint of the text colour. Markdown's code uses the same classes.
 */
export const codeRecipe = {
  inline: {
    base: 'font-mono bg-text/8',
  },
  block: {
    base: 'self-stretch overflow-hidden rounded-control bg-text/6',
  },
  header: {
    base: 'flex-row items-center gap-3 min-h-10 ps-3 pe-1.5 border-b border-text/8',
  },
  title: {
    base: 'grow basis-0 min-w-0 truncate text-footnote font-medium text-muted',
  },
  /** Long lines scroll sideways; they never wrap, since a wrapped line reads as two. */
  scroll: {
    base: 'self-stretch overflow-x-auto p-3',
    /** Tints reach the block's sides, so the inline padding moves onto the columns. */
    lined: 'self-stretch overflow-x-auto py-3',
  },
  /**
   * Numbers, signs and code side by side over the tints. Every column and every tint row is 1.5 of the
   * same font size high per line, so the rows stay under their lines in any pack and at any text size.
   */
  lines: {
    base: 'relative self-start min-w-full flex-row font-mono text-footnote',
  },
  tints: {
    base: 'absolute inset-0',
  },
  tint: {
    base: 'h-[1.5em] self-stretch',
    context: '',
    added: 'bg-success-soft',
    removed: 'bg-danger-soft',
    header: 'bg-text/6',
    highlighted: 'bg-accent-soft',
  },
  gutter: {
    base: 'shrink-0 ps-3 pe-3 border-e border-text/8 font-mono text-footnote leading-[1.5] text-end text-muted tabular-nums whitespace-pre select-none',
  },
  signs: {
    base: 'shrink-0 ps-3 font-mono text-footnote leading-[1.5] whitespace-pre select-none',
    numbered: 'ps-2',
  },
  /** A changed line's number and sign carry the change, so it reads without the tint's colour. */
  mark: {
    context: 'text-muted',
    header: 'text-muted',
    added: 'text-success',
    removed: 'text-danger',
  },
  headerLine: {
    base: 'text-muted',
  },
  text: {
    base: 'self-start font-mono text-footnote text-text whitespace-pre',
    selectable: 'select-text',
    lined: 'shrink-0 pe-3 leading-[1.5]',
    start: 'ps-3',
    afterSign: 'ps-2',
  },
} as const;
