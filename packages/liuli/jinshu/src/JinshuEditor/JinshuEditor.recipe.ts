export const jinshuEditorParts = {
  root: 'view',
  toolbar: 'view',
  toolbarInner: 'view',
  toolbarStart: 'view',
  toolbarEnd: 'view',
  toolbarSep: 'view',
  markButton: 'button',
  markGlyph: 'text',
  history: 'view',
  saved: 'view',
  link: 'view',
  math: 'view',
  findBar: 'view',
  agentBar: 'view',
  header: 'view',
  activity: 'view',
  queue: 'view',
  unanchored: 'view',
  keyboardBar: 'view',
  formatBar: 'view',
  stream: 'view',
  loading: 'text',
  canvas: 'view',
  row: 'view',
  title: 'button',
  block: 'view',
  gutter: 'view',
  mathEditor: 'view',
  textEditor: 'view',
  textBlock: 'view',
  objectBlock: 'view',
  selectionBar: 'view',
  suggestions: 'view',
  suggestion: 'view',
  slashMenu: 'view',
  insertPanel: 'view',
  tablePicker: 'view',
  trailing: 'button',
} as const;

/**
 * The editing chrome around the document: a document bar on top, a format bar at the block being
 * edited (above the keyboard on phones), a gutter of block handles on desktop, a selection bar under
 * a selected media or table block, and suggestions reviewed in place under their block. An agent's
 * changes, what it is doing and what waits for the person sit in quiet bars, never in a modal. The
 * blocks themselves keep the reader's typography.
 */
export const jinshuEditorRecipe = {
  root: {
    base: 'w-full gap-4',
    // Room for the format bar that rides above the keyboard.
    keyboardPad: 'pb-14',
  },
  /** The document bar plus the agent bar, activity and review queue, pinned together. */
  header: {
    base: 'w-full gap-2 bg-canvas',
    sticky: 'sticky top-0 z-10 pb-2',
  },
  toolbar: {
    base: 'w-full flex-row items-center justify-center bg-canvas py-1.5 border-b border-line',
  },
  toolbarInner: {
    base: 'w-full max-w-5xl flex-row items-center justify-between gap-2',
  },
  toolbarStart: {
    base: 'flex-row items-center gap-0.5',
  },
  toolbarEnd: {
    base: 'flex-row items-center gap-1 shrink-0',
  },
  toolbarSep: {
    base: 'w-px h-4 bg-text/12 mx-1 shrink-0',
  },
  markButton: {
    base: 'size-8 shrink-0 rounded-control items-center justify-center transition-colors duration-fast',
    disabled: 'text-faint',
    active: 'bg-accent-soft text-accent',
    idle: 'text-text hover:bg-text/6 active:bg-text/10',
  },
  markGlyph: {
    base: 'text-footnote leading-none',
    active: 'text-accent',
    disabled: 'text-faint',
    idle: 'text-text',
    mark: {
      bold: 'font-bold',
      italic: 'italic font-serif',
      underline: 'underline',
      strike: 'line-through',
      code: 'font-mono text-caption',
      superscript: 'text-caption font-medium',
      subscript: 'text-caption font-medium',
    },
  },
  history: {
    base: 'w-72 gap-2',
    empty: 'text-footnote text-muted py-2',
    scroll: 'max-h-72',
    list: 'gap-0.5',
    entry: 'flex-row items-center justify-between gap-2 rounded-control px-2 py-1.5',
    entryBody: 'grow min-w-0 gap-0.5',
    note: 'text-footnote truncate',
    noteUndone: 'text-faint line-through',
    noteDone: 'text-text',
    origin: 'text-caption text-muted',
  },
  saved: {
    base: 'flex-row items-center gap-1 px-1.5',
    glyph: 'size-3.5 text-success',
    label: 'text-caption text-muted',
  },
  link: {
    base: 'w-64 gap-2.5',
    text: 'w-full rounded-control border border-line-strong bg-surface px-2.5 py-1.5 text-footnote text-text',
    href: 'w-full rounded-control border border-line-strong bg-surface px-2.5 py-1.5 text-footnote font-mono text-text',
    sections: 'gap-1',
    sectionsTitle: 'text-caption text-muted',
    sectionList: 'gap-0.5',
    section: 'rounded-control px-2 py-1 text-start hover:bg-text/6',
    sectionSelected: 'bg-accent-soft',
    sectionText: 'text-footnote text-text truncate',
    actions: 'flex-row justify-end',
  },
  math: {
    base: 'w-72 gap-2.5',
    preview: 'rounded-control bg-surface-sunken p-3 min-h-14 items-center justify-center',
    source: 'w-full rounded-control border border-line-strong bg-surface px-2.5 py-1.5 font-mono text-footnote text-text',
    actions: 'flex-row justify-end gap-1.5',
  },
  findBar: {
    base: 'w-full max-w-[74ch] mx-auto flex-row flex-wrap items-center gap-2 rounded-control border border-line bg-surface px-2.5 py-2',
    field: 'grow min-w-28 basis-28 rounded-control bg-surface-sunken px-2.5 py-1 text-footnote text-text',
    count: 'text-caption tabular-nums text-muted',
  },
  /** One agent batch: who changed how much, with view and undo for that batch only. */
  agentBar: {
    base: 'w-full max-w-[74ch] mx-auto flex-row flex-wrap items-center justify-between gap-2 rounded-control bg-accent-soft/40 ps-3 pe-1.5 py-1',
    text: 'text-footnote font-medium text-text min-w-0 shrink',
    actions: 'flex-row items-center gap-0.5 shrink-0',
  },

  /** An agent reading or writing here. */
  activity: {
    base: 'w-full flex-row items-center gap-2 rounded-control bg-info-soft/40 ps-3 pe-1.5 py-1',
    top: 'max-w-[74ch] mx-auto',
    dot: 'size-2 shrink-0 rounded-full bg-info',
    text: 'grow min-w-0 text-footnote text-text truncate',
  },
  /** Everything waiting for the person, one item at a time. */
  queue: {
    base: 'w-full max-w-[74ch] mx-auto gap-1 rounded-control bg-accent-soft/40 ps-3 pe-1.5 py-1.5',
    head: 'flex-row items-center justify-between gap-2',
    nav: 'flex-row items-center gap-1 min-w-0',
    count: 'text-footnote font-medium text-text',
    item: 'flex-row flex-wrap items-center justify-between gap-2',
    body: 'grow min-w-0 gap-0.5',
    title: 'text-footnote text-text',
    detail: 'text-caption text-muted',
    actions: 'flex-row items-center gap-0.5 shrink-0',
  },
  unanchored: {
    base: 'w-full max-w-[74ch] mx-auto gap-1.5',
  },
  keyboardBar: {
    base: 'fixed start-0 end-0 z-20 flex-row items-center border-t border-line bg-surface-raised ps-1 pe-2 py-1',
    scroll: 'grow min-w-0 overflow-x-auto',
    controls: 'flex-row items-center gap-0.5 min-w-max',
  },
  formatBar: {
    base: 'absolute bottom-full start-0 z-20 mb-1.5 flex-row items-center gap-0.5 rounded-container border border-line bg-surface-raised px-1 py-0.5 shadow-raised',
  },
  stream: {
    base: 'rounded-container border border-info/30 bg-info-soft/40 p-3.5 gap-2',
  },
  loading: {
    base: 'text-footnote text-muted py-8 text-center',
  },
  canvas: {
    base: 'w-full max-w-[74ch] mx-auto py-4',
  },
  /** The gutter track and the block or title beside it. */
  row: {
    base: 'flex-row items-start gap-1.5',
    body: 'grow min-w-0',
  },
  title: {
    input: 'w-full bg-canvas/0',
    button: 'w-full text-start rounded-checkbox hover:bg-text/3',
    empty: 'text-faint',
  },
  block: {
    base: 'gap-1.5',
    findMatch: 'bg-warning-soft/50 rounded-control',
    // A change bar at the block's start edge: it reads on any block, a tinted callout included.
    agentChanged: 'absolute -start-3 top-0.5 bottom-0.5 w-[3px] rounded-full bg-accent',
    body: 'relative grow min-w-0 gap-1.5',
  },
  gutter: {
    base: 'w-16 shrink-0 flex-row items-center justify-end gap-0.5',
    spacer: 'w-16 shrink-0',
    // Lines suggestions and the slash menu up with the blocks past the gutter.
    indent: 'ps-16',
  },
  mathEditor: {
    base: 'rounded-container border border-accent/45 bg-surface-sunken p-4 gap-3 ring-1 ring-accent/25',
    scroll: 'w-full overflow-x-auto',
    preview: 'min-w-full items-center justify-center py-1',
    source: 'border-t border-text/10 pt-2.5 gap-1.5',
    field: 'w-full rounded-control bg-surface px-3 py-1.5 font-mono text-footnote text-text',
    footer: 'flex-row items-center justify-between',
    hint: 'text-caption text-faint',
  },
  textEditor: {
    base: 'relative w-full',
    field: 'w-full p-0 bg-canvas/0 text-text',
  },
  textBlock: {
    base: 'relative w-full rounded-checkbox hover:bg-text/3 transition-colors duration-fast',
    hit: 'absolute inset-0 rounded-checkbox',
  },
  objectBlock: {
    base: 'relative gap-2 rounded-container',
    selected: 'ring-2 ring-accent p-1',
    // Coarse pointer: an unselected media or table block is one target that selects it.
    hit: 'absolute inset-0 rounded-container',
  },
  /** Under the selected block: move, replace, properties, preview, delete, more. */
  selectionBar: {
    base: 'self-start flex-row flex-wrap items-center gap-0.5 rounded-control border border-line bg-surface-raised ps-1 pe-1 py-0.5',
    sep: 'w-px h-5 bg-line mx-0.5',
  },
  suggestions: {
    base: 'gap-1.5',
  },
  suggestion: {
    base: 'rounded-control border border-accent/25 bg-accent-soft/25 px-3 py-2 gap-1.5',
    header: 'flex-row flex-wrap items-center justify-between gap-2',
    meta: 'flex-row items-center gap-1.5 min-w-0',
    note: 'text-footnote text-text truncate',
    actions: 'flex-row items-center gap-1 shrink-0',
    summary: 'text-caption font-mono text-muted',
    formatOnly: 'gap-0.5',
    formatLabel: 'text-caption text-muted',
    diff: 'text-footnote leading-relaxed text-text',
    context: 'text-muted',
    removed: 'bg-danger-soft line-through text-danger',
    added: 'bg-success-soft font-medium text-success',
  },
  slashMenu: {
    base: 'rounded-container border border-line bg-surface p-2 gap-1 shadow-raised',
    hint: 'text-caption text-faint px-2 py-0.5',
    grid: 'grid grid-cols-2 md:grid-cols-3 gap-1',
    item: 'flex-row items-center gap-2 rounded-control px-2.5 py-1.5 transition-colors duration-fast',
    itemSelected: 'bg-accent-soft',
    itemIdle: 'hover:bg-text/5',
    badge: 'w-6 text-center text-caption font-mono font-semibold text-accent',
    label: 'text-footnote text-text',
  },
  insertPanel: {
    base: 'w-72 gap-3',
    section: 'gap-1',
    heading: 'text-caption font-medium text-muted px-2',
    grid: 'grid grid-cols-2 gap-0.5',
    item: 'flex-row items-center gap-2 rounded-control px-2 py-1.5 text-start hover:bg-text/6 active:bg-text/10',
    badge: 'w-6 shrink-0 text-center text-caption font-mono font-semibold text-muted',
    label: 'text-footnote text-text',
  },
  tablePicker: {
    base: 'gap-1.5 px-2',
    header: 'flex-row items-center justify-between',
    title: 'text-caption font-medium text-muted',
    size: 'text-caption tabular-nums text-muted',
    grid: 'gap-1 self-start',
    row: 'flex-row gap-1',
    cell: 'size-5 rounded-xs border transition-colors duration-fast',
    cellLit: 'bg-accent-soft border-accent',
    cellIdle: 'bg-surface border-text/15',
  },
  trailing: {
    base: 'mt-4 py-8 text-start rounded-control hover:bg-text/2',
    hint: 'text-footnote text-faint',
  },
} as const;
