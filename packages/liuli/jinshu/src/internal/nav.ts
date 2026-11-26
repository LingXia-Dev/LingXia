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

const handles = new WeakMap<object, Pick<JinshuNavHandle, 'scrollTo' | 'setActiveHeading'>>();

/**
 * Pure-data navigation handle backed by `useLocal` (no `useRef` needed).
 * Connects `<JinshuOutline>` and `<JinshuView>`. Its functions keep their identity across renders,
 * so a view it drives is not drawn again while its state holds.
 */
export function useJinshuNav(name = 'jinshu-nav'): JinshuNavHandle {
  const local = useLocal<JinshuNavState>(name, {
    targetBlockId: null,
    seq: 0,
    activeHeadingId: null,
  });
  let fns = handles.get(local);
  if (!fns) {
    fns = {
      scrollTo(blockId: string) {
        local.set({ targetBlockId: blockId, seq: local.value.seq + 1, activeHeadingId: blockId });
        try {
          reveal(blockElementId(name, blockId));
        } catch {
          // Element may be collapsed inside details; JinshuView auto-expands on targetBlockId change.
        }
      },
      setActiveHeading(headingId: string | null) {
        if (local.value.activeHeadingId === headingId) return;
        local.set({ targetBlockId: local.value.targetBlockId, seq: local.value.seq, activeHeadingId: headingId });
      },
    };
    handles.set(local, fns);
  }
  return { scope: name, state: local.value, ...fns };
}
