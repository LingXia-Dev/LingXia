export const conversationScrollButtonParts = {
  root: 'button',
  icon: 'vector',
  count: 'text',
} as const;

/**
 * A round button floating at the end corner over the bottom of a conversation, on an opaque raised fill
 * with the floating shadow; hover and press shade inside its own fill, as a pressable Card does, so the
 * messages never show through. The count of messages that arrived out of view sits on its corner in the accent.
 */
export const conversationScrollButtonRecipe = {
  root: {
    base: 'absolute bottom-3 end-3 size-10 items-center justify-center rounded-full border border-line bg-surface-raised text-text shadow-raised transition duration-fast ease-standard starting:opacity-0 starting:translate-y-2 motion-reduce:transition-none hover:border-line-strong hover:inset-ring-[999px] hover:inset-ring-text/4 active:inset-ring-[999px] active:inset-ring-text/8 focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2',
  },
  icon: {
    base: 'size-5',
  },
  count: {
    base: 'absolute -top-1.5 -end-1.5 min-w-5 h-5 px-1 rounded-full bg-accent text-on-accent text-caption font-medium text-center tabular-nums',
  },
} as const;
