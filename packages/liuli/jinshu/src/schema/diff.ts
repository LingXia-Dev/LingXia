import { blockFields } from './fields';
import { blockContentHash, plainText } from './helpers';
import type { JinshuBlock, JinshuDocument } from './types';

export type DiffMode = 'smart' | 'inline' | 'block';

export interface WordDiffSegment {
  readonly kind: 'equal' | 'insert' | 'delete';
  readonly text: string;
}

export interface AttrChange {
  readonly key: string;
  readonly before: unknown;
  readonly after: unknown;
}

export type BlockDiffEntry =
  | { readonly kind: 'unchanged'; readonly id: string; readonly block: JinshuBlock }
  | { readonly kind: 'added'; readonly id: string; readonly block: JinshuBlock; readonly toIndex: number }
  | { readonly kind: 'removed'; readonly id: string; readonly block: JinshuBlock; readonly fromIndex: number }
  | { readonly kind: 'moved'; readonly id: string; readonly block: JinshuBlock; readonly fromIndex: number; readonly toIndex: number; readonly changed?: boolean }
  | {
      readonly kind: 'modified';
      readonly id: string;
      readonly before: JinshuBlock;
      readonly after: JinshuBlock;
      readonly typeChanged: boolean;
      readonly marksOnly: boolean;
      readonly wordDiff?: readonly WordDiffSegment[];
      readonly attrChanges: readonly AttrChange[];
      readonly fullBlockReplace: boolean;
    };

export interface DocumentDiff {
  readonly mode: DiffMode;
  readonly entries: readonly BlockDiffEntry[];
  readonly stats: {
    readonly added: number;
    readonly removed: number;
    readonly modified: number;
    readonly moved: number;
    readonly unchanged: number;
  };
}

/** Splits text the same way on every engine: a Han or kana character alone, a run of letters and digits, a run of spaces, any other mark alone. */
export function defaultSegmenter(text: string): string[] {
  return text.match(/[\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}]|(?:(?![\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}])[\p{L}\p{M}\p{N}_])+|\s+|[^\s\p{L}\p{M}\p{N}_]/gu) ?? (text ? [text] : []);
}

/** Simple LCS-based word diff for block text comparison. */
export function diffWords(aText: string, bText: string, segmenter = defaultSegmenter): WordDiffSegment[] {
  if (aText === bText) return aText ? [{ kind: 'equal', text: aText }] : [];
  const a = segmenter(aText);
  const b = segmenter(bText);

  // Guard against quadratic blowup on huge blocks
  if (a.length * b.length > 200_000) {
    return [
      ...(aText ? [{ kind: 'delete' as const, text: aText }] : []),
      ...(bText ? [{ kind: 'insert' as const, text: bText }] : []),
    ];
  }

  const m = a.length;
  const n = b.length;
  const dp: Uint32Array[] = Array.from({ length: m + 1 }, () => new Uint32Array(n + 1));
  for (let i = m - 1; i >= 0; i--) {
    for (let j = n - 1; j >= 0; j--) {
      dp[i][j] = a[i] === b[j] ? dp[i + 1][j + 1] + 1 : Math.max(dp[i + 1][j], dp[i][j + 1]);
    }
  }

  const raw: WordDiffSegment[] = [];
  let i = 0;
  let j = 0;
  while (i < m && j < n) {
    if (a[i] === b[j]) {
      raw.push({ kind: 'equal', text: a[i] });
      i += 1;
      j += 1;
    } else if (dp[i + 1][j] >= dp[i][j + 1]) {
      raw.push({ kind: 'delete', text: a[i] });
      i += 1;
    } else {
      raw.push({ kind: 'insert', text: b[j] });
      j += 1;
    }
  }
  while (i < m) raw.push({ kind: 'delete', text: a[i++] });
  while (j < n) raw.push({ kind: 'insert', text: b[j++] });

  // Merge adjacent segments of same kind
  const merged: WordDiffSegment[] = [];
  for (const seg of raw) {
    const prev = merged[merged.length - 1];
    if (prev && prev.kind === seg.kind) {
      merged[merged.length - 1] = { kind: prev.kind, text: prev.text + seg.text };
    } else {
      merged.push(seg);
    }
  }
  return merged;
}

function blockPlainText(block: JinshuBlock): string | null {
  switch (block.type) {
    case 'paragraph':
    case 'heading':
      return plainText(block.runs);
    case 'code':
      return block.code;
    case 'math':
      return block.latex;
    default:
      return null;
  }
}

function diffAttrs(before: JinshuBlock, after: JinshuBlock): AttrChange[] {
  // Content and identity fields are diffed as text or whole blocks, not as attributes.
  const skip = new Set(['id', 'type']);
  for (const f of [...(blockFields(before.type) ?? []), ...(blockFields(after.type) ?? [])]) {
    if (f.structural) skip.add(f.name);
  }
  const aObj = before as unknown as Record<string, unknown>;
  const bObj = after as unknown as Record<string, unknown>;
  const keys = new Set([...Object.keys(aObj), ...Object.keys(bObj)]);
  const changes: AttrChange[] = [];
  for (const key of keys) {
    if (skip.has(key)) continue;
    if (JSON.stringify(aObj[key]) !== JSON.stringify(bObj[key])) {
      changes.push({ key, before: aObj[key], after: bObj[key] });
    }
  }
  return changes;
}

/**
 * Aligns two documents by stable block `id` and computes block/word/attr diffs.
 */
export function diffDocuments(
  a: JinshuDocument,
  b: JinshuDocument,
  options?: { readonly mode?: DiffMode; readonly segmenter?: (text: string) => string[] },
): DocumentDiff {
  const mode: DiffMode = options?.mode ?? 'smart';
  const segmenter = options?.segmenter ?? defaultSegmenter;

  const aIndexById = new Map<string, number>();
  const aBlockById = new Map<string, JinshuBlock>();
  const aHashById = new Map<string, string>();
  a.blocks.forEach((blk, idx) => {
    aIndexById.set(blk.id, idx);
    aBlockById.set(blk.id, blk);
    aHashById.set(blk.id, blockContentHash(blk));
  });

  const bIndexById = new Map<string, number>();
  b.blocks.forEach((blk, idx) => bIndexById.set(blk.id, idx));

  const entries: BlockDiffEntry[] = [];
  let added = 0;
  let removed = 0;
  let modified = 0;
  let moved = 0;
  let unchanged = 0;

  // Track which removed blocks from `a` occur before the next surviving block in `b`
  const emittedRemoved = new Set<string>();
  let aCursor = 0;

  function flushRemovedBefore(targetAIndex: number): void {
    while (aCursor < targetAIndex) {
      const blk = a.blocks[aCursor];
      if (!bIndexById.has(blk.id) && !emittedRemoved.has(blk.id)) {
        emittedRemoved.add(blk.id);
        entries.push({ kind: 'removed', id: blk.id, block: blk, fromIndex: aCursor });
        removed += 1;
      }
      aCursor += 1;
    }
  }

  let lastMatchedAIndex = -1;

  for (let bIdx = 0; bIdx < b.blocks.length; bIdx++) {
    const bBlock = b.blocks[bIdx];
    const aIdx = aIndexById.get(bBlock.id);
    if (aIdx === undefined) {
      entries.push({ kind: 'added', id: bBlock.id, block: bBlock, toIndex: bIdx });
      added += 1;
      continue;
    }

    flushRemovedBefore(aIdx);
    const aBlock = aBlockById.get(bBlock.id)!;
    const isMoved = aIdx < lastMatchedAIndex;
    if (aIdx > lastMatchedAIndex) lastMatchedAIndex = aIdx;

    const sameHash = aHashById.get(bBlock.id) === blockContentHash(bBlock);
    if (sameHash) {
      if (isMoved) {
        entries.push({ kind: 'moved', id: bBlock.id, block: bBlock, fromIndex: aIdx, toIndex: bIdx, changed: false });
        moved += 1;
      } else {
        entries.push({ kind: 'unchanged', id: bBlock.id, block: bBlock });
        unchanged += 1;
      }
      continue;
    }

    const typeChanged = aBlock.type !== bBlock.type;
    const aText = blockPlainText(aBlock);
    const bText = blockPlainText(bBlock);
    const attrChanges = diffAttrs(aBlock, bBlock);

    let wordDiff: WordDiffSegment[] | undefined;
    let marksOnly = false;
    let fullBlockReplace = mode === 'block';

    if (aText !== null && bText !== null) {
      if (aText === bText && !typeChanged && attrChanges.length === 0) {
        marksOnly = true;
      } else if (mode !== 'block') {
        wordDiff = diffWords(aText, bText, segmenter);
        if (mode === 'smart') {
          const equalChars = wordDiff.filter(s => s.kind === 'equal').reduce((acc, s) => acc + s.text.length, 0);
          const maxLen = Math.max(1, aText.length, bText.length);
          if (equalChars / maxLen < 0.5) {
            fullBlockReplace = true;
          }
        }
      }
    } else if (mode === 'smart') {
      fullBlockReplace = true;
    }

    entries.push({
      kind: 'modified',
      id: bBlock.id,
      before: aBlock,
      after: bBlock,
      typeChanged,
      marksOnly,
      wordDiff,
      attrChanges,
      fullBlockReplace,
    });
    modified += 1;
  }

  flushRemovedBefore(a.blocks.length);

  return {
    mode,
    entries,
    stats: { added, removed, modified, moved, unchanged },
  };
}
