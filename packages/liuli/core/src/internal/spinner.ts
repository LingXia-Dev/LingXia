import type { VectorPath } from '@lingxia/view';

/** A quarter arc on a 24-unit grid, spun by `animate-spin`. It is a motion mark, not an icon, so no set redraws it. */
export const spinnerBox = [0, 0, 24, 24] as const;
export const spinner: VectorPath[] = [{ d: 'M21 12a9 9 0 0 0-9-9', fill: 'none', stroke: 'currentColor', strokeWidth: 2.5, strokeLinecap: 'round', strokeLinejoin: 'round' }];
