import type { FieldSize, FieldStatusType } from '../Field/Field';

export const comboboxParts = {
  trigger: 'button',
  editor: 'input',
  value: 'text',
  chevron: 'vector',
  clear: 'button',
  popover: 'view',
  sheet: 'dialog',
  grabber: 'view',
  header: 'view',
  title: 'text',
  close: 'button',
  list: 'scroll',
  option: 'button',
  optionText: 'view',
  optionLabel: 'text',
  optionDescription: 'text',
  optionCheck: 'vector',
  note: 'text',
  spinner: 'vector',
} as const;

/**
 * The field is Select's box. With a finger it is a button that opens a sheet on the bottom edge
 * holding a search field over the options. With a mouse and keyboard the box is the editor, and
 * the options hang under it while focus stays in the text. Options are rows that wash on hover
 * and focus, as menu items do.
 */
export const comboboxRecipe = {
  trigger: {
    base: 'flex-row items-center gap-2 px-3 rounded-control border border-line-strong bg-surface text-start transition duration-fast ease-standard hover:border-muted',
    // The sheet's trigger is a button; the typeahead's box holds the editor and takes its focus ring.
    pressable: 'focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2 disabled:bg-text/5 disabled:border-text/8',
    editable: 'focus-within:border-accent focus-within:inset-ring-1 focus-within:inset-ring-accent',
    disabled: 'bg-text/5 border-text/8',
    size: { sm: 'min-h-control-sm', md: 'min-h-control-md', lg: 'min-h-control-lg' } satisfies Record<FieldSize, string>,
    status: { error: 'border-danger', warning: 'border-warning', success: 'border-line-strong' } satisfies Record<FieldStatusType, string>,
  },
  value: {
    base: 'min-w-0 grow basis-0 truncate text-text',
    placeholder: 'min-w-0 grow basis-0 truncate text-muted',
    disabled: 'text-faint',
    size: { sm: 'text-footnote', md: 'text-body', lg: 'text-callout' } satisfies Record<FieldSize, string>,
  },
  editor: {
    base: 'grow basis-0 min-w-0 truncate self-stretch px-0 bg-transparent text-text placeholder:text-muted disabled:text-faint',
  },
  chevron: {
    // Select's chevron as the engine draws it: 10 by 5 pt with a 1.75 pt stroke, in the same lane.
    base: 'size-5 shrink-0 text-muted',
    disabled: 'text-faint',
  },
  /**
   * `hasClear`: a button that empties the field, beside the value and before the chevron, so the chevron
   * keeps its place. Where the box is itself a button it lies over the box, in a lane the value keeps free.
   */
  clear: {
    base: 'size-8 shrink-0 items-center justify-center rounded-full text-muted hover:bg-text/5 active:bg-text/8 focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2',
    icon: 'size-4',
    wrap: 'relative',
    lane: 'me-8',
    dock: 'absolute inset-y-0 flex-row items-center',
  },
  popover: {
    // As wide as the field once that is measured. It holds the suggestions' scroll, which the View never
    // scales, so it slides 4 px out from the field instead of growing.
    typeahead: 'w-72 max-w-[90vw] p-1 rounded-container border border-line bg-surface-overlay shadow-raised transition duration-fast ease-standard starting:opacity-0 exit:opacity-0 motion-safe:starting:-translate-y-1',
  },
  sheet: {
    base: 'mt-auto mb-0 w-full max-w-[640px] max-h-[90vh] px-4 pt-2 pb-4 gap-3 rounded-t-dialog border border-b-0 border-line bg-surface-overlay shadow-modal transition duration-slow ease-spring motion-safe:starting:translate-y-full motion-safe:exit:translate-y-full motion-reduce:starting:opacity-0 motion-reduce:exit:opacity-0',
  },
  grabber: {
    base: 'self-center w-9 h-1 rounded-full bg-text/20',
  },
  header: {
    base: 'flex-row items-start gap-3 min-h-8 px-1',
  },
  title: {
    base: 'grow basis-0 pt-1 text-title font-semibold tracking-tight text-text',
  },
  close: {
    base: 'size-8 -me-1.5 items-center justify-center rounded-full text-muted hover:bg-text/5 active:bg-text/8 focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2',
    icon: 'size-5',
  },
  list: {
    popover: 'max-h-64 shrink min-h-0 overscroll-contain',
    sheet: 'max-h-[50vh] shrink min-h-0 overscroll-contain',
    box: 'gap-0.5',
  },
  option: {
    base: 'flex-row items-center gap-3 px-3 py-2 rounded-control text-start text-text hover:bg-text/5 active:bg-text/8 focus-visible:bg-text/8 disabled:opacity-40',
    size: { sm: 'min-h-control-sm', md: 'min-h-control-md', lg: 'min-h-control-lg' } satisfies Record<FieldSize, string>,
    // The option the arrow keys are on, while focus stays in the text.
    active: 'bg-text/8',
  },
  optionText: {
    base: 'min-w-0 grow basis-0',
  },
  optionLabel: {
    base: 'text-body',
  },
  optionDescription: {
    base: 'text-footnote text-muted',
  },
  optionCheck: {
    base: 'size-4 shrink-0 text-accent',
  },
  note: {
    base: 'px-3 py-3 text-footnote text-muted',
  },
  spinner: {
    base: 'mx-3 my-3 size-4 text-muted motion-safe:animate-spin',
  },
} as const;
