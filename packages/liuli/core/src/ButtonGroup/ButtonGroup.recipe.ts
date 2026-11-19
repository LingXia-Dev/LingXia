export const buttonGroupParts = { root: 'view' } as const;

/**
 * A row of buttons. Apart, they keep a gap; attached, they touch and share edges, and each
 * button rounds only the group's two ends (that part is in Button's recipe).
 */
export const buttonGroupRecipe = {
  root: {
    base: 'flex-row flex-wrap items-center self-start max-w-full',
    apart: 'gap-2',
    // A line that no longer fits wraps rather than run past its card; the rows keep a gap between them.
    attached: 'gap-x-0 gap-y-2',
  },
} as const;
