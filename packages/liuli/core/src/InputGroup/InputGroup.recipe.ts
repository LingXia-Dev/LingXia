import type { FieldSize } from '../Field/Field';

export const inputGroupParts = { root: 'view', text: 'view', textLabel: 'text' } as const;

/**
 * Fields, selects, buttons and fixed text joined edge to edge into one control: neighbours share an
 * edge and only the group's two ends are round (those parts are in each control's recipe).
 */
export const inputGroupRecipe = {
  root: {
    base: 'flex-row items-stretch self-stretch',
  },
  /** Fixed text joined to a field: "https://", "@example.com", "kg". Sunken and quiet, so it is read as part of the value, not as something to press. */
  text: {
    base: 'shrink-0 flex-row items-center px-3 border border-line-strong bg-surface-sunken',
    size: { sm: 'min-h-control-sm', md: 'min-h-control-md', lg: 'min-h-control-lg' } satisfies Record<FieldSize, string>,
  },
  textLabel: {
    base: 'text-muted whitespace-nowrap tabular-nums',
    size: { sm: 'text-footnote', md: 'text-body', lg: 'text-callout' } satisfies Record<FieldSize, string>,
  },
  /** A text field takes the room the others leave. */
  grows: 'grow basis-0 min-w-0',
} as const;
