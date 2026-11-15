export const codeParts = {
  inline: 'span',
  block: 'view',
  header: 'view',
  title: 'text',
  scroll: 'scroll',
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
  },
  text: {
    base: 'self-start font-mono text-footnote text-text whitespace-pre',
    selectable: 'select-text',
  },
} as const;
