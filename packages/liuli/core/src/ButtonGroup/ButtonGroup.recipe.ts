export const buttonGroupParts = { root: 'view' } as const;

/**
 * A row of buttons. Apart, they keep a gap; attached, they touch and share edges, and each
 * button rounds only the group's two ends (that part is in Button's recipe).
 */
export const buttonGroupRecipe = {
  root: {
    base: 'flex-row items-center self-start',
    apart: 'flex-wrap gap-2',
    attached: 'gap-0',
  },
} as const;
