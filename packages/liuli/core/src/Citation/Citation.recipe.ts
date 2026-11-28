export const citationParts = {
  group: 'view',
  content: 'view',
  mark: 'link',
  sources: 'view',
  source: 'button',
  sourceNumber: 'text',
  sourceName: 'text',
  card: 'view',
  cardTitle: 'text',
  cardSite: 'text',
  cardExcerpt: 'text',
} as const;

/**
 * A cited answer: a small numbered mark among the words for each claim, and the sources as a row of
 * pills under them. A mark shows its source's card on hover, focus or a tap; a pill opens the source.
 */
export const citationRecipe = {
  group: {
    base: 'gap-2',
  },
  /** The cited words, as the app laid them out. */
  content: {
    base: 'gap-2',
  },
  /** A run, so it takes only colour and type: a small accent number, as a reference reads in print. */
  mark: {
    base: 'text-caption font-semibold tabular-nums text-accent no-underline',
  },
  sources: {
    base: 'flex-row flex-wrap gap-1.5',
  },
  source: {
    base: 'shrink min-w-0 max-w-full flex-row items-center gap-1.5 min-h-7 ps-1 pe-2.5 rounded-full border border-line bg-surface transition-colors duration-fast ease-standard hover:bg-text/4 active:bg-text/7 focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2',
  },
  sourceNumber: {
    base: 'min-w-5 px-1 rounded-full text-center text-caption font-medium tabular-nums text-on-accent-soft bg-accent-soft',
  },
  sourceName: {
    base: 'shrink min-w-0 text-footnote text-muted truncate',
  },
  /** HoverCard's card: the title, where it is from, and the words it was cited for. */
  card: {
    base: 'w-72 mt-1.5 p-4 gap-1 rounded-container border border-line bg-surface-overlay shadow-raised transition duration-fast ease-standard starting:opacity-0 exit:opacity-0',
  },
  cardTitle: {
    base: 'text-callout font-medium text-text',
  },
  cardSite: {
    base: 'text-footnote text-muted',
  },
  cardExcerpt: {
    base: 'mt-1 text-footnote text-text line-clamp-4',
  },
} as const;
