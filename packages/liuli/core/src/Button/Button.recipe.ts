import type { ButtonEdge, ButtonSize, ButtonVariant, ButtonWidth } from './Button';

/** The element each part lands on. */
export const buttonParts = { root: 'button', glyph: 'vector', icon: 'view', spinner: 'vector', label: 'text', reason: 'text' } as const;

/**
 * Button classes by part, then by prop value. Exported so an app can style a raw `<button>` (a
 * `popoverTarget` invoker, say) to match.
 */
export const buttonRecipe = {
  root: {
    // Neighbours never squeeze a button into wrapping its label; only a button wider than its whole row wraps it.
    base: 'flex-row items-center justify-center-safe shrink-0 max-w-full rounded-control transition-colors duration-fast ease-standard focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2',
    variant: {
      primary: 'bg-accent-solid text-on-accent-solid border border-transparent shadow-[0_1px_2px_rgb(0_0_0/0.10)] hover:bg-accent-solid/92 active:bg-accent-solid/88',
      secondary: 'bg-surface text-text border border-text/12 shadow-[0_1px_2px_rgb(0_0_0/0.05)] hover:bg-text/5 active:bg-text/8',
      ghost: 'bg-transparent text-text border border-transparent hover:bg-text/5 active:bg-text/8',
      link: 'bg-transparent text-accent border border-transparent hover:bg-accent/6 active:bg-accent/10',
      destructive: 'bg-danger-solid text-on-danger-solid border border-transparent shadow-[0_1px_2px_rgb(0_0_0/0.10)] hover:bg-danger-solid/92 active:bg-danger-solid/88',
    } satisfies Record<ButtonVariant, string>,
    /**
     * Disabled drops the variant's colour for a faint ink fill and muted text, which stays above
     * 4.5:1 in both themes; fading the variant would leave its label below 3:1.
     */
    disabled: {
      primary: 'disabled:bg-text/6 disabled:text-muted disabled:border-transparent disabled:shadow-none',
      secondary: 'disabled:bg-text/6 disabled:text-muted disabled:border-transparent disabled:shadow-none',
      ghost: 'disabled:text-muted',
      link: 'disabled:text-muted',
      destructive: 'disabled:bg-text/6 disabled:text-muted disabled:border-transparent disabled:shadow-none',
    } satisfies Record<ButtonVariant, string>,
    size: {
      sm: 'min-h-control-sm px-3 gap-1.5',
      md: 'min-h-control-md px-4 gap-2',
      lg: 'min-h-control-lg px-5 gap-2',
    } satisfies Record<ButtonSize, string>,
    /** Square, for a trigger that shows only its icon. */
    square: {
      sm: 'size-control-sm px-0',
      md: 'size-control-md px-0',
      lg: 'size-control-lg px-0',
    } satisfies Record<ButtonSize, string>,
    /** Over scrolling content: the raised elevation in place of the variant's hairline shadow. */
    floating: 'shadow-raised',
    width: {
      content: 'self-start',
      fill: 'self-stretch',
    } satisfies Record<ButtonWidth, string>,
    // Padding plus the 1 pt border hang past the edge; an icon adds its drawing's inset (5/24 of its box). A square hangs its
    // inset around the glyph: (square − glyph) / 2 plus that drawing inset.
    edge: {
      start: {
        label: { sm: '-ms-[13px]', md: '-ms-[17px]', lg: '-ms-[21px]' },
        icon: { sm: '-ms-[16.33px]', md: '-ms-[20.75px]', lg: '-ms-[25.17px]' },
        square: { sm: '-ms-[11.33px]', md: '-ms-[14.75px]', lg: '-ms-[18.17px]' },
      },
      end: {
        label: { sm: '-me-[13px]', md: '-me-[17px]', lg: '-me-[21px]' },
        icon: { sm: '-me-[16.33px]', md: '-me-[20.75px]', lg: '-me-[25.17px]' },
        square: { sm: '-me-[11.33px]', md: '-me-[14.75px]', lg: '-me-[18.17px]' },
      },
    } satisfies Record<ButtonEdge, Record<'label' | 'icon' | 'square', Record<ButtonSize, string>>>,
    /**
     * In an attached group: neighbours share an edge and only the group's ends are round. The focus
     * ring is drawn inside the button, where the next button cannot cover it.
     */
    attached: 'rounded-none first:rounded-s-control last:rounded-e-control -ms-px first:ms-0 focus-visible:ring-0 focus-visible:inset-ring-2 focus-visible:inset-ring-focus',
  },
  glyph: {
    size: { sm: 'size-4 shrink-0', md: 'size-[1.125rem] shrink-0', lg: 'size-5 shrink-0' } satisfies Record<ButtonSize, string>,
  },
  icon: {
    base: 'items-center justify-center',
  },
  spinner: {
    base: 'motion-safe:animate-spin',
    size: { sm: 'size-4', md: 'size-4', lg: 'size-5' } satisfies Record<ButtonSize, string>,
  },
  label: {
    base: 'text-start',
    /** A link reads as text in a sentence, so it keeps the regular weight. */
    weight: { primary: 'font-medium', secondary: 'font-medium', ghost: 'font-medium', link: 'font-normal', destructive: 'font-medium' } satisfies Record<ButtonVariant, string>,
    size: { sm: 'text-footnote', md: 'text-body', lg: 'text-callout' } satisfies Record<ButtonSize, string>,
  },
  /** The disabled reason: not drawn, read as the description. */
  reason: {
    base: 'sr-only',
  },
} as const;
