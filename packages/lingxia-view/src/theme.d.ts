import type { ColorRole } from './generated/authoring.js';
import type { PaletteSlot } from './generated/palette.js';

export { defaultPalette } from './generated/palette.js';
export type { PaletteSlot } from './generated/palette.js';
export type { ColorRole } from './generated/authoring.js';

/** The role `role` draws as inside an element whose nearest surface background is `surface`, as native paints it: `line` and `line-strong` inside `surface-raised` or `surface-overlay` take that surface's pair. */
export declare function roleUnder(role: ColorRole, surface: ColorRole): PaletteSlot;
