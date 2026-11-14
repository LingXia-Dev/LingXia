import { createContext } from '@lingxia/view';

/**
 * The rung a container sits on: `page` under the canvas, `overlay` inside a popover, dialog or sheet.
 * A list or card inside an overlay takes the overlay's own surface, never a darker one.
 */
export type Layer = 'page' | 'overlay';
export const LayerContext = createContext<Layer>('page');
