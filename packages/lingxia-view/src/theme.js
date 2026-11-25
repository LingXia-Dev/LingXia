import { surfaceLines } from './generated/contract.js';

export { defaultPalette } from './generated/palette.js';

/** The role `role` draws as inside an element whose nearest surface background is `surface`, as native paints it. */
export function roleUnder(role, surface) {
  return surfaceLines[surface]?.[role] ?? role;
}
