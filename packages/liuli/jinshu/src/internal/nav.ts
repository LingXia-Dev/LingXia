import { reveal, useLocal, type VisibleChangeEvent } from '@lingxia/view';
import type { JinshuDocument } from '../schema/types';

/** A block's element id in the instance `scope`; ids are only unique per page, so each instance scopes its own. */
export function blockElementId(scope: string, blockId: string): string {
  return `jinshu-${scope}-b-${blockId}`;
}

/**
 * The id of the nearest element that carries one for `id`: blocks, list items, table rows and columns
 * have their own; an inline atom resolves to the block, item or row that holds it.
 */
export function revealTargetId(doc: JinshuDocument, id: string): string | undefined {
  const visit = (node: unknown, owner: string | undefined): string | undefined => {
    if (Array.isArray(node)) {
      for (const child of node) {
        const hit = visit(child, owner);
        if (hit) return hit;
      }
      return undefined;
    }
    if (!node || typeof node !== 'object') return undefined;
    const rec = node as Record<string, unknown>;
    const isAtom = typeof rec.inline === 'string';
    if (rec.id === id) return isAtom ? owner : id;
    const next = !isAtom && typeof rec.id === 'string' ? rec.id : owner;
    for (const key in rec) {
      if (key === 'attrs') continue;
      const value = rec[key];
      if (value && typeof value === 'object') {
        const hit = visit(value, next);
        if (hit) return hit;
      }
    }
    return undefined;
  };
  return visit(doc.blocks, undefined);
}

/**
 * Follows an in-page `#id` link: resolves `id` with `revealTargetId`, then scrolls through `nav` or reveals
 * the element in `scope`. Returns the resolved id, or undefined when nothing in `doc` holds `id`.
 */
export function scrollToInPage(
  doc: JinshuDocument,
  id: string,
  scope: string,
  nav?: Pick<JinshuNavHandle, 'scrollTo'>,
): string | undefined {
  const resolved = revealTargetId(doc, id);
  if (!resolved) return undefined;
  if (nav) nav.scrollTo(resolved);
  else reveal(blockElementId(scope, resolved));
  return resolved;
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
  /** For the `<scroll>` holding the reader: tracks the heading being read and feeds `onObserve`. */
  onVisibleChange(event: VisibleChangeEvent): void;
}

type NavFns = Pick<JinshuNavHandle, 'scrollTo' | 'setActiveHeading' | 'onVisibleChange'>;
const handles = new WeakMap<object, NavFns>();
// The reader a nav drives, keyed by the nav's stable `scrollTo`; set while it renders.
const readers = new WeakMap<object, (event: VisibleChangeEvent) => void>();

/** Routes the nav's scroll visibility to the reader it drives. */
export function setNavReader(nav: JinshuNavHandle, reader: (event: VisibleChangeEvent) => void): void {
  readers.set(nav.scrollTo, reader);
}

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
    const own: NavFns = {
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
      onVisibleChange(event: VisibleChangeEvent) {
        const reader = readers.get(own.scrollTo);
        if (!reader) return;
        try {
          reader(event);
        } catch {
          // The reader unmounted; its state cells are gone.
          readers.delete(own.scrollTo);
        }
      },
    };
    fns = own;
    handles.set(local, fns);
  }
  return { scope: name, state: local.value, ...fns };
}
