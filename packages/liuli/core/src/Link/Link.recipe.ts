export const linkParts = {
  root: 'link',
} as const;

/** The accent colour and an underline: colour alone does not tell a link from the words around it. */
export const linkRecipe = {
  root: {
    base: 'text-accent underline active:text-accent/70',
    disabled: 'text-faint underline',
  },
} as const;
