import { reveal, useLocal } from '@lingxia/view';

/** A block's element id in the instance `scope`; ids are only unique per page, so each instance scopes its own. */
export function blockElementId(scope: string, blockId: string): string {
  return `jinshu-${scope}-b-${blockId}`;
}

export interface JinshuNavState {
  readonly targetBlockId: string | null;
  readonly seq: number;
  readonly activeHeadingId: string | null;
}

export interface JinshuNavHandle {
  /** Scopes the element ids of the `JinshuView` it drives, so several documents can share a page. */
  readonly scope: string;
  readonly state: JinshuNavState;
  scrollTo(blockId: string): void;
  setActiveHeading(headingId: string | null): void;
}

/**
 * Pure-data navigation handle backed by `useLocal` (no `useRef` needed).
 * Connects `<JinshuOutline>` and `<JinshuView>`.
 */
export function useJinshuNav(name = 'jinshu-nav'): JinshuNavHandle {
  const local = useLocal<JinshuNavState>(name, {
    targetBlockId: null,
    seq: 0,
    activeHeadingId: null,
  });

  return {
    scope: name,
    get state() {
      return local.value;
    },
    scrollTo(blockId: string) {
      const nextSeq = local.value.seq + 1;
      local.set({
        targetBlockId: blockId,
        seq: nextSeq,
        activeHeadingId: blockId,
      });
      try {
        reveal(blockElementId(name, blockId));
      } catch {
        // Element may be collapsed inside details; JinshuView auto-expands on targetBlockId change.
      }
    },
    setActiveHeading(headingId: string | null) {
      if (local.value.activeHeadingId === headingId) return;
      local.set({
        targetBlockId: local.value.targetBlockId,
        seq: local.value.seq,
        activeHeadingId: headingId,
      });
    },
  };
}
