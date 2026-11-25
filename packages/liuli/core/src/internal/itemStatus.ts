import { createContext } from '@lingxia/view';

/**
 * Inside an Item: a label-hidden control's failure goes to the row's status line, under its words. The
 * control keeps announcing and describing it itself; the row only draws it.
 */
export const ItemStatusContext = createContext<((message: string | null) => void) | undefined>(undefined);
