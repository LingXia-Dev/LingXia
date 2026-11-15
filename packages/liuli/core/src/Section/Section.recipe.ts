export const sectionParts = {
  root: 'view',
  header: 'view',
  heading: 'view',
  title: 'text',
  description: 'text',
  action: 'view',
} as const;

/** A heading over its content: the title a step above body, the action level with it at the end. */
export const sectionRecipe = {
  root: {
    base: 'self-stretch gap-3',
  },
  header: {
    base: 'flex-row items-end gap-3',
  },
  heading: {
    base: 'grow shrink basis-0 min-w-0 gap-0.5',
  },
  title: {
    base: 'font-semibold text-text',
    level: { 2: 'text-callout tracking-tight', 3: 'text-body', 4: 'text-footnote text-muted' } satisfies Record<2 | 3 | 4, string>,
  },
  description: {
    base: 'text-footnote text-muted',
  },
  action: {
    base: 'shrink-0 flex-row items-center gap-2',
  },
} as const;
