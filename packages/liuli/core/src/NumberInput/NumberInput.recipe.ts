import type { FieldSize } from '../Field/Field';

export const numberInputParts = {
  box: 'view',
  adornment: 'text',
  input: 'input',
  steppers: 'view',
  stepper: 'button',
  stepperIcon: 'vector',
} as const;

/**
 * The box is TextInput's. The number is set in tabular figures; the two steppers sit at the end
 * of the box behind a hairline, each a square as tall as the box, so they are easy to hit and
 * never look like part of the number.
 */
export const numberInputRecipe = {
  box: {
    base: 'pe-0 overflow-hidden',
  },
  input: {
    base: 'tabular-nums',
  },
  steppers: {
    base: 'flex-row self-stretch border-s border-text/12',
  },
  stepper: {
    base: 'items-center justify-center self-stretch text-muted transition-colors duration-fast ease-standard hover:bg-text/5 active:bg-text/8 focus-visible:bg-text/8 disabled:text-faint',
    size: { sm: 'w-8', md: 'w-10', lg: 'w-12' } satisfies Record<FieldSize, string>,
    second: 'border-s border-text/12',
  },
  stepperIcon: {
    base: 'size-4',
  },
} as const;
