/** Display formulas set from views: serif type, hairline rules and brackets drawn as borders. */
export const mathRecipe = {
  root: {
    base: 'flex-row flex-wrap items-center justify-center gap-y-1.5',
  },
  text: {
    base: 'font-serif text-callout text-text',
    compact: 'font-serif text-footnote text-text',
    italic: 'italic',
    bold: 'font-semibold',
  },
  row: {
    baseline: 'flex-row items-baseline',
  },
  // A plus laid out unseen holds the width; the face's U+2212 is drawn centred over it.
  minus: {
    base: 'justify-center',
    ghost: 'opacity-0',
    glyph: 'absolute inset-0 text-center',
  },
  // TeX's thick (5/18 em) and medium (4/18 em) spaces at each size.
  space: {
    rel: { base: 'mx-1.25', compact: 'mx-1' },
    bin: { base: 'mx-1', compact: 'mx-0.75' },
  },
  bracket: {
    squareOpen: 'w-1.5 self-stretch border-y border-s border-text my-0.5',
    squareClose: 'w-1.5 self-stretch border-y border-e border-text my-0.5',
    bar: 'w-px self-stretch bg-text my-0.5 mx-1',
    doubleBar: 'flex-row gap-0.5 self-stretch my-0.5 mx-1',
    barLine: 'w-px self-stretch bg-text',
    // The container radius is the default pack's xl step, the curve these parentheses were drawn with.
    roundOpen: 'w-2 self-stretch border-s-2 border-text rounded-s-container my-0.5',
    roundClose: 'w-2 self-stretch border-e-2 border-text rounded-e-container my-0.5',
    glyph: 'font-serif text-display text-text/80 px-0.5',
  },
  frac: {
    base: 'items-center justify-center px-1 gap-0.5',
    part: 'flex-row items-baseline justify-center',
    bar: 'w-full h-px bg-text',
    binomial: 'flex-row items-center px-0.5',
  },
  sqrt: {
    base: 'flex-row items-stretch',
    index: 'justify-start pe-0.5',
    sign: 'pe-0.5',
    body: 'flex-row items-baseline border-t border-text px-0.5',
  },
  overline: {
    base: 'flex-row items-baseline border-t border-text px-0.5',
  },
  underline: {
    base: 'flex-row items-baseline border-b border-text px-0.5',
  },
  xarrow: {
    base: 'items-center justify-center px-1.5',
  },
  bigop: {
    base: 'flex-row items-center pe-1 gap-0.5',
    textOp: 'items-center justify-center px-1',
    symbol: 'font-serif text-display text-text',
    limits: 'justify-between gap-1',
  },
  script: {
    base: 'flex-row',
    text: 'font-serif text-caption text-text',
    raised: 'items-start',
    lowered: 'items-end',
    centred: 'items-center',
    both: 'justify-between gap-1',
    supOnly: '-mt-1',
    subOnly: '-mb-1',
  },
  env: {
    base: 'flex-row items-center px-1 gap-1',
    rows: 'gap-1.5 py-0.5',
    row: 'flex-row items-center',
    rowAligned: 'gap-4',
    rowMatrix: 'gap-5',
    cell: 'flex-row items-center',
    cellAligned: 'justify-start',
    cellMatrix: 'justify-center min-w-5',
  },
} as const;
