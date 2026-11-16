export const videoControlsParts = {
  root: 'view',
  surface: 'button',
  centre: 'view',
  start: 'button',
  loading: 'view',
  error: 'view',
  chrome: 'view',
  bar: 'view',
  scrubber: 'slider',
  row: 'view',
  button: 'button',
  glyph: 'vector',
  time: 'text',
  live: 'text',
  menu: 'view',
  chip: 'button',
  fullscreen: 'dialog',
} as const;

/**
 * Over the picture only the media roles paint, in both appearances: the accent stays off the video.
 * Glass is the `media` fill without a backdrop, which would be recaptured every frame over a moving
 * picture.
 * The scrubber's touch box ends just under its line and grows up into the picture, so none of it
 * lies over the buttons; its top padding puts the line there. Pressed, the track thickens and the
 * thumb shows.
 */
export const videoControlsRecipe = {
  root: {
    overlay: 'absolute inset-0 text-on-media',
    bar: 'gap-1 text-text',
  },
  edges: {
    base: 'absolute text-on-media',
  },
  surface: {
    base: 'absolute inset-0 bg-transparent',
    under: 'z-0',
    over: 'z-[2]',
  },
  centre: {
    base: 'absolute inset-0 items-center justify-center-safe',
    under: 'z-[1]',
    over: 'z-[2]',
  },
  disc: {
    // The hairline keeps media glass visible on a black letterbox as on a bright picture.
    base: 'items-center justify-center rounded-full border border-on-media/20 bg-media text-on-media',
    inline: 'size-[60px]',
    full: 'size-[72px]',
  },
  spinner: {
    inline: 'size-7 animate-spin',
    full: 'size-8 animate-spin',
  },
  start: {
    base: 'ps-0.5',
  },
  error: {
    base: 'max-w-[280px] items-center gap-2.5 rounded-dialog bg-media px-[18px] py-3.5',
    text: 'text-center text-body font-medium text-on-media',
    retry: 'h-11 min-w-24 items-center justify-center rounded-full bg-on-media/25 px-[18px]',
    retryText: 'text-body font-semibold text-on-media',
  },
  chrome: {
    base: 'absolute inset-0 z-[1] transition-opacity duration-base ease-standard',
    shown: 'opacity-100',
    hidden: 'opacity-0',
  },
  bar: {
    inline: 'absolute start-2 end-2 bottom-2 rounded-container border border-on-media/20 bg-media px-0.5 pt-[14px]',
    full: 'absolute start-4 end-4 bottom-3 rounded-dialog border border-on-media/20 bg-media px-0.5 pt-4 pb-1',
  },
  scrubber: {
    base: 'absolute start-3 end-3 px-0 pb-0 track:rounded-full track:bg-on-media/25 secondary:rounded-full secondary:bg-on-media/45 range:rounded-full range:bg-on-media thumb:size-3 thumb:rounded-full thumb:bg-transparent track:h-[3px] secondary:h-[3px] range:h-[3px] active:track:h-[5px] active:secondary:h-[5px] active:range:h-[5px] active:thumb:bg-knob focus-visible:thumb:bg-on-media focus-visible:thumb:ring-2 focus-visible:thumb:ring-focus',
    inline: '-top-[30px] h-11 pt-10',
    full: '-top-8 h-12 pt-11',
  },
  row: {
    base: 'flex-row items-center justify-between',
    group: 'shrink-0 flex-row items-center',
  },
  button: {
    base: 'items-center justify-center bg-transparent focus-visible:ring-2 focus-visible:ring-focus',
    inline: 'size-11 rounded-container',
    full: 'size-12 rounded-container',
    overlay: 'text-on-media',
    bar: 'text-text',
  },
  glyph: {
    inline: 'size-5',
    full: 'size-[22px]',
    disc: 'size-[30px]',
    discFull: 'size-9',
  },
  time: {
    base: 'ps-0.5 tabular-nums text-on-media',
    inline: 'text-caption',
    full: 'text-[13px]',
    total: 'text-on-media/70',
    bar: 'shrink-0 ps-3.5 pe-3 text-caption tabular-nums text-muted',
  },
  live: {
    base: 'ps-1 text-caption font-semibold',
    overlay: 'text-on-media',
    bar: 'text-accent',
  },
  close: {
    inline: 'absolute top-3 start-2 rounded-full border border-on-media/20 bg-media',
    full: 'absolute top-3 start-4 rounded-full border border-on-media/20 bg-media',
  },
  menuLayer: {
    base: 'absolute inset-0 z-[3]',
    dismiss: 'absolute inset-0 bg-transparent',
    // The menu takes the bar's place, so the two never overlap on a small player.
    inline: 'absolute start-2 end-2 bottom-2',
    full: 'absolute start-4 end-4 bottom-3',
  },
  menu: {
    base: 'overflow-hidden border border-on-media/20 bg-media',
    inline: 'rounded-container',
    full: 'rounded-dialog',
    body: 'gap-2 p-2',
    // One row per setting, the chips sharing its width.
    group: 'flex-row gap-1.5',
  },
  chip: {
    base: 'h-11 min-w-0 grow basis-0 items-center-safe justify-center rounded-container px-1 focus-visible:ring-2 focus-visible:ring-focus',
    overlay: { off: 'bg-on-media/25', on: 'bg-on-media', textOff: 'font-normal text-on-media', textOn: 'font-semibold text-media' },
    page: { off: 'bg-text/8', on: 'bg-accent', textOff: 'font-normal text-text', textOn: 'font-semibold text-on-accent' },
    text: 'tabular-nums',
    textInline: 'text-footnote',
    textFull: 'text-body',
  },
  barRow: {
    // No gap: each icon's own inset and the time's padding give one 24-unit space between glyphs.
    base: 'flex-row items-center',
    // Inset as the row's glyphs are, and as far from the picture above as from the glyphs below.
    menu: 'gap-1.5 px-3 pt-3',
    error: 'flex-row items-center gap-2 px-3',
    errorText: 'grow basis-0 text-caption text-danger',
    // Slider's own md control, so a player on a page matches the Slider beside it.
    slider: 'mx-0.5 grow basis-0 h-control-md p-0 text-accent track:h-1 track:rounded-full track:bg-line range:h-1 range:rounded-full range:bg-accent/80 secondary:h-1 secondary:rounded-full secondary:bg-line-strong/50 thumb:size-5 thumb:rounded-full thumb:bg-knob thumb:border-2 thumb:border-accent thumb:shadow-contact hover:thumb:scale-105 active:thumb:scale-110 focus-visible:thumb:ring-2 focus-visible:thumb:ring-focus focus-visible:thumb:ring-offset-2 disabled:range:bg-accent/40 disabled:thumb:border-accent/40 disabled:thumb:shadow-none',
  },
  fullscreen: {
    // The video fills the panel and backs itself in black.
    panel: 'm-0 h-full max-h-full w-full max-w-full rounded-none bg-transparent p-0',
    stage: 'relative grow justify-center',
    video: 'w-full grow object-contain',
  },
} as const;
