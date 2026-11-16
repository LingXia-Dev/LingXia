import type { StepperOrientation } from './Stepper';

type State = 'done' | 'current' | 'upcoming';

export const stepperParts = {
  root: 'view',
  step: 'view',
  marker: 'view',
  number: 'text',
  line: 'view',
  label: 'text',
  description: 'text',
} as const;

/**
 * Numbered markers joined by a line. A finished step is a filled tick and its line is filled; the
 * step in progress is ringed in the accent with its label in the text colour; what is ahead is quiet.
 */
export const stepperRecipe = {
  root: {
    base: 'self-stretch',
    orientation: { horizontal: 'flex-row items-start', vertical: '' } satisfies Record<StepperOrientation, string>,
  },
  item: {
    grow: 'grow basis-0',
    row: 'min-w-0',
  },
  step: {
    base: 'self-stretch text-start',
    orientation: { horizontal: 'gap-1.5', vertical: 'flex-row items-stretch gap-3' } satisfies Record<StepperOrientation, string>,
    pressable: 'rounded-control transition-colors duration-fast ease-standard hover:bg-text/4 active:bg-text/7 focus-visible:ring-2 focus-visible:ring-focus',
  },
  rail: {
    row: 'flex-row items-center self-stretch',
    column: 'items-center',
  },
  marker: {
    base: 'size-6 shrink-0 items-center justify-center rounded-full',
    state: {
      done: 'bg-accent text-on-accent',
      current: 'inset-ring-2 inset-ring-accent',
      upcoming: 'inset-ring-1 inset-ring-line-strong',
    } satisfies Record<State, string>,
    tick: 'size-3.5',
  },
  number: {
    base: 'text-caption font-semibold tabular-nums',
    state: { done: '', current: 'text-accent', upcoming: 'text-muted' } satisfies Record<State, string>,
  },
  line: {
    base: 'rounded-full',
    orientation: { horizontal: 'grow h-0.5 mx-1.5', vertical: 'grow w-0.5 my-1 min-h-3' } satisfies Record<StepperOrientation, string>,
    done: 'bg-accent',
    ahead: 'bg-text/12',
  },
  body: {
    column: 'grow shrink basis-0 min-w-0 pt-0.5',
    gap: 'pb-4',
  },
  text: {
    base: 'gap-0.5',
    orientation: { horizontal: 'pe-2', vertical: '' } satisfies Record<StepperOrientation, string>,
  },
  label: {
    base: 'text-footnote',
    state: { done: 'text-text', current: 'font-semibold text-text', upcoming: 'text-muted' } satisfies Record<State, string>,
    row: 'truncate',
  },
  description: {
    base: 'text-footnote text-muted',
  },
} as const;
