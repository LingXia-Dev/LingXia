/** The in-place editors for structured blocks; typography comes from the document's recipe. */
export const blockEditorsRecipe = {
  keptNote: 'text-caption text-faint',
  list: {
    root: 'gap-1.5',
    styleBar: 'self-start flex-row items-center gap-1 rounded-container border border-line bg-surface px-1.5 py-1 shadow-raised',
    styleHint: 'text-caption text-faint ps-1',
    item: 'flex-row items-start gap-2.5',
    tick: 'size-3 text-on-accent',
    body: 'grow min-w-0',
    input: 'w-full p-0 bg-canvas/0',
    itemButton: 'w-full text-start rounded-checkbox hover:bg-text/3',
    checked: 'text-muted',
    empty: 'text-faint',
  },
  container: {
    preview: 'gap-1.5',
    muted: 'text-muted',
    heading: 'font-semibold',
    other: 'text-caption font-mono text-faint',
  },
  callout: {
    preview: 'relative hover:ring-1 hover:ring-text/10',
    previewButton: 'absolute inset-0 rounded-container',
    editing: 'gap-2 ring-1 ring-accent/40',
    header: 'flex-row flex-wrap items-center justify-between gap-2',
    titleInput: 'grow min-w-0 bg-canvas/0',
    tones: 'flex-row items-center gap-1',
    body: 'w-full p-0 bg-canvas/0',
    footer: 'flex-row items-center justify-between gap-2',
  },
  quote: {
    preview: 'relative rounded-checkbox hover:bg-text/3',
    previewButton: 'absolute inset-0 rounded-checkbox',
    editing: 'gap-1',
    body: 'w-full bg-canvas/0',
    bodyText: 'text-muted',
  },
  details: {
    // The reader's summary row: same height, inset and chevron.
    header: 'flex-row items-center gap-3 min-h-control-lg py-2.5 -mx-3 px-3 rounded-control',
    summary: 'grow min-w-0 bg-canvas/0 text-body font-medium text-text',
    toggle: 'size-8 -me-1.5 shrink-0 items-center justify-center rounded-control hover:bg-text/4 active:bg-text/7',
    chevron: {
      base: 'size-5 text-muted transition-transform duration-base ease-standard',
      open: '-rotate-180',
    },
    body: 'pb-4 gap-1',
    bodyInput: 'w-full p-0 bg-canvas/0',
  },
  code: {
    editing: 'ring-1 ring-accent/40',
    fields: 'grow min-w-0 flex-row items-center gap-1.5',
    filename: 'grow min-w-0 bg-canvas/0 p-0 text-caption font-mono font-medium text-text',
    language: 'w-20 bg-canvas/0 p-0 text-caption font-mono font-medium text-muted',
    // The reader's body padding; the field wraps nothing, so each number stays on its line.
    body: 'self-stretch py-3',
    lines: 'self-stretch flex-row items-start',
    input: 'grow min-w-0 p-0 ps-3.5 pe-3.5 bg-canvas/0 font-mono text-footnote leading-[1.5] text-text whitespace-pre',
  },
  table: {
    root: 'relative',
    // Floats above the table so showing it moves nothing.
    toolbarDock: 'absolute bottom-full start-0 end-0 mb-1.5 z-20 items-start',
    toolbar: 'self-start max-w-full overflow-x-auto rounded-control border border-line bg-surface px-1.5 py-0.5 shadow-raised',
    toolbarRow: 'flex-row items-center gap-0.5 min-w-max',
    toolbarLabel: 'text-caption font-medium text-muted px-1.5',
    toolbarSeparator: 'w-px h-4 bg-text/12 mx-0.5',
    cell: {
      selected: 'bg-accent-soft/45',
      doomed: 'bg-danger-soft',
      editing: 'bg-surface inset-ring-2 inset-ring-accent',
    },
    // The reader's cell text, as a field.
    input: {
      header: 'w-full p-0 bg-canvas/0 text-footnote font-semibold text-text',
      text: 'w-full p-0 bg-canvas/0 text-footnote text-text',
    },
    cellButton: 'w-full text-start',
    cellEmpty: 'text-faint',
    colHandle: 'absolute top-0 inset-x-0 h-2.5 items-center pt-0.5',
    rowHandle: 'absolute start-0 inset-y-0 w-2.5 justify-center ps-0.5',
    handle: {
      base: 'rounded-full transition-colors duration-fast',
      col: 'h-1 w-6',
      row: 'w-1 h-5',
      idle: 'bg-text/15',
      // Out of sight until the table is hovered or touched, still a target.
      hidden: 'bg-text/0',
      selected: 'bg-accent',
      doomed: 'bg-danger',
    },
  },
  columns: {
    stacked: 'gap-4',
    side: 'flex-row items-start gap-5',
    column: 'min-w-0 gap-2 rounded-control p-1',
    input: 'w-full bg-canvas/0',
    child: 'relative rounded-checkbox hover:bg-text/3',
    childButton: 'absolute inset-0 rounded-checkbox',
  },
  inspector: {
    root: 'rounded-control border border-line bg-surface-sunken p-3 gap-2',
    choices: 'flex-row flex-wrap items-center gap-1.5',
    label: 'text-caption text-muted',
    field: 'w-full rounded-checkbox bg-surface px-2.5 py-1.5 text-footnote text-text',
    src: 'text-caption font-mono text-faint truncate',
  },
} as const;
