export const sectionParts = {
  root: 'view',
  header: 'view',
  heading: 'view',
  title: 'text',
  description: 'text',
  action: 'view',
  actionControls: 'view',
} as const;

/** A heading over its content: the title a step above body, the action centred on its first line at the end. */
export const sectionRecipe = {
  root: {
    base: 'self-stretch gap-3',
  },
  header: {
    base: 'flex-row items-start gap-3',
    /** At large text: the action under the heading, at the start. */
    stacked: 'items-start gap-2',
  },
  heading: {
    base: 'grow shrink basis-0 min-w-0 gap-0.5',
    stacked: 'self-stretch gap-0.5',
  },
  title: {
    base: 'font-semibold text-text',
    level: { 2: 'text-callout tracking-tight', 3: 'text-body', 4: 'text-footnote text-muted' } satisfies Record<2 | 3 | 4, string>,
  },
  description: {
    base: 'text-footnote text-muted whitespace-pre-line',
  },
  /** As tall as the title's first line at any text size: a strut in the title's style sets it. */
  action: {
    base: 'shrink-0 flex-row items-center',
    stacked: 'items-start gap-2',
  },
  /** No height of its own, so a taller control stands evenly above and below that line. */
  actionControls: {
    base: 'h-0 flex-row items-center gap-2',
  },
} as const;
