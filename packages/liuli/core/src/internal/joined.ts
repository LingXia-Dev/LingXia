import { createContext, useContext } from '@lingxia/view';

/** Where a control sits in an InputGroup: only the group's two ends are round. */
export type JoinedEdge = 'start' | 'middle' | 'end' | 'only';

export interface Joined {
  edge: JoinedEdge;
  /** The group's size, for a control that sets none. */
  size: 'sm' | 'md' | 'lg';
}

export const JoinedContext = createContext<Joined | null>(null);

/** Where this control sits in its InputGroup and the group's size, or null outside one. */
export const useJoined = (): Joined | null => useContext(JoinedContext);

/** A joined control's corners: square where it meets a neighbour. */
export const joinedCorners: Record<JoinedEdge, string> = {
  start: 'rounded-none rounded-s-control',
  middle: 'rounded-none',
  end: 'rounded-none rounded-e-control',
  only: '',
};

/**
 * A joined control's own box in the row: each after the first draws its start edge over the edge
 * before it. A focused field keeps its accent whole there, as its second pixel is an inset ring.
 */
export const joinedPlace: Record<JoinedEdge, string> = {
  start: '',
  middle: '-ms-px',
  end: '-ms-px',
  only: '',
};
