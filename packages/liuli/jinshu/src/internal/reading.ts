import type { JinshuBlock, JinshuDocument } from '../schema/types';

/** One top-level block on screen: `ratio` its share shown, `top` its start from the viewport's, in points. */
export interface SeenBlock {
  readonly id: string;
  readonly ratio: number;
  readonly top: number;
}

interface OpenDwell {
  since: number;
  seen: number;
  idle: number;
}

/** What a reader remembers between two visibility reports. */
export interface ReadingState {
  at: number | null;
  readonly open: Map<string, OpenDwell>;
  position: string;
}

export const newReadingState = (): ReadingState => ({ at: null, open: new Map(), position: '' });

export type ReadingFact =
  | { readonly kind: 'dwell'; readonly blockId: string; readonly dwellMs: number; readonly seenRatio: number; readonly idleMs: number }
  | { readonly kind: 'position'; readonly blockId: string; readonly offset: number };

// A block still on screen reports what it gathered once this much has; a block leaving always does.
const DWELL_FLUSH_MS = 1000;

/**
 * Folds one visibility report into `state`: dwell for blocks that left or gathered a second, the
 * reading position when it moved, which blocks left the screen and the heading being read.
 * The View sees only scrolling, so `idleMs` is the longest stretch without a report.
 */
export function readVisibility(
  state: ReadingState,
  doc: JinshuDocument,
  at: number,
  shown: readonly SeenBlock[],
): { readonly facts: ReadingFact[]; readonly left: string[]; readonly headingId: string | null | undefined } {
  const gap = state.at === null ? 0 : Math.max(0, at - state.at);
  state.at = at;
  const onScreen = new Map(shown.filter(s => s.ratio > 0).map(s => [s.id, s]));
  const facts: ReadingFact[] = [];
  const left: string[] = [];

  for (const [id, open] of state.open) {
    const dwell = at - open.since;
    const idle = Math.max(open.idle, gap);
    const still = onScreen.get(id);
    if (still) open.seen = Math.max(open.seen, still.ratio);
    if (!still || dwell >= DWELL_FLUSH_MS) {
      if (dwell > 0) facts.push({ kind: 'dwell', blockId: id, dwellMs: Math.round(dwell), seenRatio: round2(open.seen), idleMs: Math.round(Math.min(idle, dwell)) });
      if (still) state.open.set(id, { since: at, seen: still.ratio, idle: 0 });
      else {
        state.open.delete(id);
        left.push(id);
      }
    } else {
      open.idle = idle;
    }
  }
  for (const [id, s] of onScreen) if (!state.open.has(id)) state.open.set(id, { since: at, seen: s.ratio, idle: 0 });

  // The block being read: the last one at or past the top, else the first on screen.
  const ordered = [...onScreen.values()];
  let current: SeenBlock | undefined;
  for (const s of ordered) if (s.top <= 0) current = s;
  current ??= ordered[0];
  let headingId: string | null | undefined;
  if (current) {
    const offset = Math.max(0, Math.round(-current.top));
    const key = `${current.id}\u0000${offset}`;
    if (key !== state.position) {
      state.position = key;
      facts.push({ kind: 'position', blockId: current.id, offset });
    }
    headingId = headingAtOrBefore(doc.blocks, current.id);
  }
  return { facts, left, headingId };
}

function headingAtOrBefore(blocks: readonly JinshuBlock[], id: string): string | null {
  let heading: string | null = null;
  for (const block of blocks) {
    if (block.type === 'heading') heading = block.id;
    if (block.id === id) return heading;
  }
  return null;
}

const round2 = (n: number) => Math.round(Math.min(1, Math.max(0, n)) * 100) / 100;
