import type { ImageSize } from '../schema/types';
import type { CodeTokenKind } from '../internal/codeTokens';

export const jinshuViewParts = {
  root: 'view',
  docTitle: 'text',
  resume: 'view',
  agentBar: 'view',
  activity: 'view',
  heading: 'text',
  paragraph: 'text',
  quote: 'view',
  callout: 'view',
  list: 'view',
  code: 'view',
  math: 'view',
  table: 'view',
  divider: 'view',
  image: 'view',
  video: 'view',
  audio: 'view',
  file: 'view',
  columns: 'view',
  extension: 'view',
} as const;

/** The document's typography and rhythm, in Liuli's theme roles and scale names. */
export const jinshuViewRecipe = {
  root: {
    base: 'w-full max-w-[65ch] gap-0',
    wide: 'w-full gap-0',
  },
  skeleton: {
    base: 'gap-4 py-2',
    title: 'w-2/3',
    lines: 'w-full',
    block: 'w-full h-36 rounded-container',
  },
  unavailable: {
    base: 'text-footnote text-muted',
  },
  resume: {
    base: 'mb-5 flex-row items-center gap-1 rounded-control bg-accent-soft/50 ps-3 pe-1 py-1',
  },
  resumeText: {
    base: 'grow min-w-0 truncate text-footnote text-text',
  },
  /** The agent bar and whole-document marks, held at the top of the page's scroll while reading. */
  agentHeader: {
    base: 'sticky top-0 z-10 gap-2 bg-canvas pb-3',
  },
  /** An agent's latest change: who, how much, View and Dismiss; the editor's bar without Undo. */
  agentBar: {
    base: 'w-full flex-row flex-wrap items-center justify-between gap-2 rounded-control bg-accent-soft/40 ps-3 pe-1.5 py-1',
    text: 'text-footnote font-medium text-text min-w-0 shrink',
    actions: 'flex-row items-center gap-0.5 shrink-0',
  },
  /** An agent writing here; nothing to press in a reader. */
  activity: {
    base: 'w-full flex-row items-center gap-2 rounded-control bg-info-soft/40 px-3 py-1.5',
    // Above the block it marks, which keeps its own gap below.
    block: 'mt-4',
    dot: 'size-2 shrink-0 rounded-full bg-info',
    text: 'grow min-w-0 text-footnote text-text truncate',
  },
  /** A change bar at a changed block's start edge, as in the editor. */
  agentChanged: {
    base: 'absolute -start-3 top-0.5 bottom-0.5 w-0.75 rounded-full bg-accent',
  },
  docTitle: {
    base: 'text-display font-semibold tracking-tight text-text mb-4',
  },
  heading: {
    base: 'font-semibold tracking-tight text-text',
    level: {
      1: 'text-display',
      2: 'text-title',
      3: 'text-callout',
      4: 'text-body',
    },
  },
  paragraph: {
    base: 'text-body leading-relaxed text-text',
  },
  run: {
    bold: 'font-semibold',
    italic: 'italic',
    underline: 'underline',
    strike: 'line-through',
    // Translucent so the chip reads on a tinted callout or highlight in both themes.
    code: 'font-mono text-footnote bg-text/8 text-text',
    sub: 'text-caption',
    super: 'text-caption',
    highlight: {
      accent: 'bg-accent-soft text-text',
      warning: 'bg-warning-soft text-text',
      success: 'bg-success-soft text-text',
      danger: 'bg-danger-soft text-text',
    },
    link: 'text-accent underline',
    math: 'font-serif italic text-body text-text',
    atom: 'text-footnote font-medium bg-accent-soft text-accent',
  },
  quote: {
    base: 'border-s-2 border-accent ps-4 py-0.5 gap-2 text-muted',
  },
  callout: {
    base: 'rounded-container p-4 gap-2',
    body: 'gap-1',
    tone: {
      info: 'bg-info-soft',
      tip: 'bg-success-soft',
      warning: 'bg-warning-soft',
      danger: 'bg-danger-soft',
    },
    title: {
      info: 'text-footnote font-semibold text-info',
      tip: 'text-footnote font-semibold text-success',
      warning: 'text-footnote font-semibold text-warning',
      danger: 'text-footnote font-semibold text-danger',
    },
  },
  list: {
    base: 'gap-1.5',
    item: 'flex-row items-start gap-2',
    marker: 'text-body leading-relaxed text-faint shrink-0 min-w-4 text-center',
    body: 'grow min-w-0 gap-1.5',
    checkbox: 'size-4 mt-1 rounded-checkbox border items-center justify-center shrink-0',
    checkboxDone: 'bg-accent border-accent',
    checkboxOpen: 'border-line-strong bg-surface',
    tick: 'size-3 text-on-accent',
    done: 'text-muted',
  },
  code: {
    root: 'rounded-container border border-line bg-surface-sunken overflow-hidden',
    header: 'flex-row items-center justify-between px-3.5 py-2 border-b border-line bg-surface',
    lang: 'text-caption font-mono font-medium text-muted',
    lineText: 'text-footnote font-mono text-text whitespace-pre',
    body: 'self-stretch overflow-x-auto py-3',
    lines: 'relative self-start min-w-full flex-row font-mono text-footnote',
    // Each tint row is `h-[1.5em]`, matching `leading-[1.5]` on the same text size, so it holds at any text scale.
    marks: 'absolute inset-0',
    mark: 'h-[1.5em] self-stretch',
    markOn: 'bg-accent-soft border-s-2 border-accent',
    numbers: 'shrink-0 ps-3.5 pe-3 border-e border-line font-mono text-footnote leading-[1.5] text-end text-muted tabular-nums whitespace-pre select-none',
    text: 'shrink-0 ps-3.5 pe-3.5 font-mono text-footnote leading-[1.5] text-text whitespace-pre',
    token: {
      plain: '',
      keyword: 'text-accent font-medium',
      string: 'text-success',
      comment: 'text-faint italic',
      number: 'text-warning',
      type: 'text-info',
      attr: 'text-muted',
    } satisfies Record<CodeTokenKind, string>,
  },
  math: {
    root: 'py-2 items-center justify-center',
    scroll: 'w-full overflow-x-auto',
    inner: 'min-w-full items-center justify-center',
    label: 'text-caption font-mono text-faint self-end mb-1',
    latex: 'font-mono text-callout text-text text-center',
  },
  table: {
    frame: 'rounded-container border border-line bg-surface overflow-hidden',
    scroll: 'w-full overflow-x-auto',
    // The run-under edge fades: past the end at rest, past both mid-scroll.
    more: {
      end: 'mask-[linear-gradient(to_left,transparent_0,#000_1.5rem)] rtl:mask-[linear-gradient(to_right,transparent_0,#000_1.5rem)]',
      start: 'mask-[linear-gradient(to_right,transparent_0,#000_1.5rem)] rtl:mask-[linear-gradient(to_left,transparent_0,#000_1.5rem)]',
      both: 'mask-[linear-gradient(to_right,transparent_0,#000_1.5rem),linear-gradient(to_left,transparent_0,#000_1.5rem)]',
    },
    grid: 'w-full',
    headerRow: 'flex-row border-b border-line bg-surface-sunken',
    row: 'flex-row border-b border-line',
    cell: {
      box: 'relative px-3 py-2 justify-center',
      header: 'text-footnote font-semibold text-text select-none',
      text: 'text-footnote text-text select-none',
    },
  },
  divider: {
    base: 'h-px w-full bg-line',
  },
  media: {
    // The figure's gap spaces it, the same under an image as under a video.
    caption: 'text-footnote text-muted text-center',
    errorBox: 'rounded-container border border-line bg-surface-sunken p-6 items-center justify-center gap-2',
    error: 'text-footnote text-muted',
  },
  image: {
    figure: 'gap-2',
    size: { small: 'w-1/2 self-center', medium: 'w-3/4 self-center', full: 'w-full' } satisfies Record<ImageSize, string>,
    frame: 'rounded-container overflow-hidden border border-line bg-surface-sunken w-full',
    picture: 'size-full',
  },
  video: {
    base: 'w-full gap-2',
    frame: 'relative overflow-hidden rounded-container bg-surface-sunken',
    picture: 'w-full aspect-video',
    seekNote: 'text-caption text-faint',
  },
  chapters: {
    base: 'self-stretch overflow-x-auto',
    row: 'flex-row gap-1 min-w-max',
  },
  file: {
    card: 'flex-row items-center justify-between gap-3 rounded-container border border-line bg-surface p-3.5',
    badge: 'px-2 py-1 rounded-segment bg-accent-soft text-caption font-mono font-semibold text-accent shrink-0',
    info: 'grow min-w-0 gap-0.5',
    name: 'text-body font-medium text-text',
    meta: 'text-footnote text-muted',
    main: 'flex-row items-center gap-3 grow min-w-0',
    actions: 'flex-row items-center gap-2 shrink-0',
  },
  audio: {
    card: 'rounded-container border border-line bg-surface p-4 gap-3 relative overflow-hidden',
    // Plays the source with no picture; the card is the player.
    player: 'absolute top-0 start-0 size-0 opacity-0',
    row: 'flex-row items-center gap-3',
    toggle: 'size-10 shrink-0 rounded-full items-center justify-center bg-accent-solid text-on-accent-solid active:opacity-80',
    toggleGlyph: 'size-5',
    main: 'grow min-w-0 gap-1',
    // Title, rate and time share one baseline.
    header: 'flex-row items-baseline justify-between gap-2',
    title: 'text-body font-medium text-text truncate',
    meta: 'flex-row items-baseline gap-2 shrink-0',
    rateButton: 'rounded-segment bg-surface-sunken px-1.5 py-0.5 hover:bg-text/8',
    rateText: 'text-caption font-medium tabular-nums text-muted',
    time: 'text-caption tabular-nums text-muted',
    slider: 'w-full',
    errorRow: 'flex-row items-center justify-between gap-2',
    error: 'text-caption text-danger',
    transcript: 'gap-2',
  },
  details: {
    body: 'gap-1',
  },
  columns: {
    base: 'flex-row items-start gap-5',
    stacked: 'gap-4',
    column: 'min-w-0',
  },
  extension: {
    base: 'rounded-container border border-dashed border-line bg-surface-sunken p-3.5 gap-1',
    type: 'text-caption font-mono text-faint',
    failed: 'text-caption font-mono text-danger',
    fallback: 'text-footnote text-muted',
  },
  highlightUpdate: {
    base: 'bg-accent-soft/60 rounded-control transition-colors duration-slow',
  },
  selectable: 'select-text',
  /** The gap above a block, by what precedes it. */
  spacing: {
    heading: 'mt-7',
    headingAfterHeading: 'mt-3',
    afterHeading: 'mt-2.5',
    divider: 'mt-5',
    spacious: 'mt-4',
    text: 'mt-3',
  },
} as const;
