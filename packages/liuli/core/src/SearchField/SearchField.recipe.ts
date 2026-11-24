import type { FieldSize } from '../Field/Field';

export const searchFieldParts = {
  root: 'view',
  spinner: 'vector',
  results: 'text',
  natural: 'text',
} as const;

/**
 * A search field is a TextInput with the magnifier and a clear button, so its box is TextInput's
 * recipe. What it adds is the spinner that takes the magnifier's place while a search runs, and
 * the line under it that says what was found.
 */
export const searchFieldRecipe = {
  /** Fills a column; in a row it takes its natural width and may shrink below it. */
  root: {
    base: 'self-stretch min-w-0',
  },
  /**
   * Unseen, no taller than nothing: twenty characters of the field's text plus its edges and magnifier,
   * so the field's natural width is a native search input's (about 20 characters) and grows with the text.
   */
  natural: {
    base: 'h-0 overflow-hidden opacity-0 whitespace-nowrap ps-[37px] pe-[13px]',
    size: { sm: 'text-footnote', md: 'text-body', lg: 'text-callout' } satisfies Record<FieldSize, string>,
  },
  spinner: {
    base: 'size-4 motion-safe:animate-spin',
  },
  results: {
    base: 'mt-1.5 text-footnote text-muted tabular-nums',
    hidden: 'sr-only',
  },
} as const;
