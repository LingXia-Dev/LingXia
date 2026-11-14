import type { FieldSize } from '../Field/Field';
import type { RadioGroupVariant } from './RadioGroup';

export const radioGroupParts = {
  field: 'Field',
  group: 'view',
  option: 'button',
  circle: 'view',
  dot: 'view',
  text: 'view',
  label: 'text',
  description: 'text',
} as const;

/** A Field around a radio group: one row per option, a circle drawn from the choice. */
export const radioGroupRecipe = {
  group: {
    base: '',
    variant: { list: 'gap-1', card: 'gap-2' } satisfies Record<RadioGroupVariant, string>,
  },
  option: {
    base: 'flex-row items-start gap-3 text-start disabled:opacity-40',
    variant: {
      /** A row: keyboard focus is a fill, with room past the circle and text. */
      list: '-mx-2 px-2 py-1.5 rounded-control transition-colors duration-150 ease-out hover:bg-text/4 focus-visible:bg-text/7',
      card: 'p-4 rounded-container border bg-surface transition-colors duration-150 ease-out hover:bg-text/3 active:bg-text/6 focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2',
    } satisfies Record<RadioGroupVariant, string>,
    /** A card's edge carries the choice: the accent ring, beside the filled circle. */
    card: {
      on: 'border-accent ring-1 ring-accent',
      off: 'border-text/12',
    } satisfies Record<'on' | 'off', string>,
  },
  circle: {
    base: 'items-center justify-center rounded-full border-2 transition-colors duration-150 ease-out',
    /** Centred on the label's first line. */
    size: { sm: 'size-4 mt-px', md: 'size-5 mt-px', lg: 'size-6' } satisfies Record<FieldSize, string>,
    state: {
      on: 'bg-accent border-accent',
      off: 'bg-surface border-line-strong shadow-[0_1px_2px_rgb(0_0_0/0.05)]',
    } satisfies Record<'on' | 'off', string>,
    invalid: 'border-danger',
  },
  dot: {
    base: 'rounded-full bg-on-accent transition duration-150 ease-out starting:scale-50 starting:opacity-0',
    size: { sm: 'size-1.5', md: 'size-2', lg: 'size-2.5' } satisfies Record<FieldSize, string>,
  },
  text: {
    base: 'grow shrink min-w-0 gap-0.5',
  },
  label: {
    base: 'text-text',
    variant: { list: '', card: 'font-medium' } satisfies Record<RadioGroupVariant, string>,
    size: { sm: 'text-footnote', md: 'text-body', lg: 'text-base' } satisfies Record<FieldSize, string>,
  },
  description: {
    base: 'text-footnote text-muted',
  },
} as const;
