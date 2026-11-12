import type { VectorPath } from '@lingxia/view';

// The few glyphs components draw themselves, on a 24-unit grid (`glyphBox`).

export const glyphBox = [0, 0, 24, 24] as const;

const line = { fill: 'none', stroke: 'currentColor', strokeWidth: 2, strokeLinecap: 'round', strokeLinejoin: 'round' } as const;

export const glyphs = {
  /** A quarter arc; spun by `animate-spin`. */
  spinner: [{ d: 'M21 12a9 9 0 0 0-9-9', ...line, strokeWidth: 2.5 }],
  check: [{ d: 'M5 12.5l4.5 4.5L19 7.5', ...line, strokeWidth: 2.5 }],
  chevronDown: [{ d: 'M6 9l6 6 6-6', ...line }],
  /** Points toward the inline end in LTR; mirror it with the recipe's `rtl:` class. */
  chevronEnd: [{ d: 'M9 6l6 6-6 6', ...line }],
  close: [{ d: 'M6 6l12 12M18 6L6 18', ...line }],
  error: [
    { d: 'M12 2.75a9.25 9.25 0 1 0 0 18.5a9.25 9.25 0 1 0 0-18.5z', ...line },
    { d: 'M12 7.5v5.5', ...line },
    { d: 'M12 16.5v0.01', ...line, strokeWidth: 2.5 },
  ],
  warning: [
    { d: 'M12 3.5L21.5 20H2.5z', ...line },
    { d: 'M12 9.5v4.5', ...line },
    { d: 'M12 17v0.01', ...line, strokeWidth: 2.5 },
  ],
  success: [
    { d: 'M12 2.75a9.25 9.25 0 1 0 0 18.5a9.25 9.25 0 1 0 0-18.5z', ...line },
    { d: 'M8 12.5l3 3 5-6', ...line },
  ],
} as const satisfies Record<string, readonly VectorPath[]>;
