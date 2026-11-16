export const calendarParts = {
  root: 'view',
  header: 'view',
  title: 'text',
  grid: 'view',
  week: 'view',
  weekday: 'text',
  slot: 'view',
  day: 'button',
  dayLabel: 'text',
} as const;

/**
 * A month as seven equal columns of 40 px days. The chosen day is a filled circle; a span is a
 * soft band between two of them; today is a ring. Nothing else is coloured.
 */
export const calendarRecipe = {
  root: {
    base: 'self-stretch gap-2',
  },
  header: {
    base: 'flex-row items-center gap-1 ps-2',
  },
  title: {
    base: 'grow basis-0 text-body font-semibold text-text',
  },
  grid: {
    base: 'self-stretch gap-0.5',
  },
  week: {
    base: 'flex-row self-stretch',
  },
  weekday: {
    base: 'grow basis-0 py-1 text-caption font-medium text-muted text-center',
  },
  // A day's seventh of the row: the band of a span fills it, so the band runs unbroken along a week.
  slot: {
    base: 'grow basis-0 h-10 items-center justify-center',
    between: 'bg-accent-soft',
    start: 'bg-accent-soft rounded-s-full',
    end: 'bg-accent-soft rounded-e-full',
  },
  day: {
    base: 'size-10 items-center justify-center rounded-full transition-colors duration-fast ease-standard disabled:opacity-40 focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2',
    idle: 'hover:bg-text/6 active:bg-text/10',
    chosen: 'bg-accent hover:bg-accent/92 active:bg-accent/88',
    today: 'inset-ring-1 inset-ring-line-strong',
  },
  dayLabel: {
    base: 'text-body text-text tabular-nums',
    chosen: 'font-semibold text-on-accent',
  },
} as const;
