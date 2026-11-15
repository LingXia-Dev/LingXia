import type { VectorPath } from '@lingxia/view';
import type { IconName } from './names';

/** One icon on a 24-unit grid. */
export interface IconDefinition {
  /** Path data drawn with the icon's stroke; an entry given as a whole path keeps its own paint (a filled dot). */
  readonly paths: readonly (string | VectorPath)[];
  /** The icon points along the reading direction (a chevron, a send arrow) and is flipped in right-to-left layouts. */
  readonly isMirrored?: boolean;
}

/** An icon for every name Liuli's components ask for. */
export type IconSet = Readonly<Record<IconName, IconDefinition>>;
