import { createContext } from '@lingxia/view';

/** The edge a part puts its link actions on: a link there hangs its padding past it, as `edge` does. */
export const ActionEdge = createContext<'start' | 'end' | null>(null);
