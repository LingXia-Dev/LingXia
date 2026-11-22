export const topBarParts = {
  root: 'view',
  leading: 'view',
  center: 'view',
  title: 'text',
  trailing: 'view',
} as const;

export const topBarRecipe = {
  root: {
    base: 'flex-row items-center gap-2 min-h-[44px] px-3 border-b border-text/8 bg-surface shrink-0',
  },
  leading: {
    base: 'flex-row items-center gap-1 shrink-0',
  },
  center: {
    base: 'grow min-w-0 flex-row items-center justify-center',
  },
  title: {
    base: 'text-body font-semibold text-text truncate',
  },
  trailing: {
    base: 'flex-row items-center gap-1 shrink-0',
  },
} as const;
