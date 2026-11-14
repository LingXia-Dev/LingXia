import type { ButtonSize, ButtonVariant, ButtonWidth } from './Button';

/** The element each part lands on. */
export const buttonParts = { root: 'button', icon: 'view', spinner: 'vector', label: 'text', reason: 'text' } as const;

/**
 * Button classes by part, then by prop value. Exported so an app can style a raw `<button>` (a
 * `popoverTarget` invoker, say) to match.
 */
export const buttonRecipe = {
  root: {
    base: 'flex-row items-center justify-center-safe rounded-control transition-colors duration-150 ease-out focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2',
    variant: {
      primary: 'bg-accent text-on-accent border border-transparent shadow-[0_1px_2px_rgb(0_0_0/0.10)] hover:bg-accent/92 active:bg-accent/88',
      secondary: 'bg-surface text-text border border-text/12 shadow-[0_1px_2px_rgb(0_0_0/0.05)] hover:bg-text/5 active:bg-text/8',
      ghost: 'bg-transparent text-text border border-transparent hover:bg-text/5 active:bg-text/8',
      link: 'bg-transparent text-accent border border-transparent hover:bg-accent/6 active:bg-accent/10',
      destructive: 'bg-danger text-on-danger border border-transparent shadow-[0_1px_2px_rgb(0_0_0/0.10)] hover:bg-danger/92 active:bg-danger/88',
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
    width: {
      content: 'self-start',
      fill: 'self-stretch',
    } satisfies Record<ButtonWidth, string>,
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
    size: { sm: 'text-footnote', md: 'text-body', lg: 'text-base' } satisfies Record<ButtonSize, string>,
  },
  /** The disabled reason: not drawn, read as the description. */
  reason: {
    base: 'sr-only',
  },
} as const;
