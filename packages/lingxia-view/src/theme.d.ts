import type { Role } from './generated/palette.js';

export { defaultPalette } from './generated/palette.js';
export type { Role } from './generated/palette.js';

/** The role `role` draws as inside an element whose nearest surface background is `surface`, as native paints it: `line` and `line-strong` inside `surface-raised` or `surface-overlay` take that surface's pair. */
export declare function roleUnder(role: Role, surface: Role): Role;
