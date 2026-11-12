import type { FieldSize, FieldStatusType } from '../Field/Field';

export const selectParts = { field: 'Field', control: 'select' } as const;

/**
 * The select primitive is the box: border, focus ring and padding sit on it. It draws a muted
 * chevron 10 px in from its end padding, keeps the label out of that lane and cuts a long label
 * short on one line.
 */
export const selectRecipe = {
  control: {
    base: 'px-3 rounded-control border border-line-strong bg-surface text-text placeholder:text-muted transition duration-150 ease-out focus:border-accent focus:ring-3 focus:ring-accent/20 disabled:bg-text/5 disabled:border-line disabled:text-faint',
    size: {
      sm: 'min-h-control-sm text-footnote',
      md: 'min-h-control-md text-body',
      lg: 'min-h-control-lg text-base',
    } satisfies Record<FieldSize, string>,
    status: { error: 'border-danger', warning: 'border-warning', success: 'border-line-strong' } satisfies Record<FieldStatusType, string>,
  },
} as const;
