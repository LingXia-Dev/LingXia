import {
  cx,
  reveal,
  useChannel,
  useId,
  useLeaveGuard,
  useLocal,
  useView,
  type Child,
} from '@lingxia/view';
import { Badge } from '@liulidesign/core/Badge';
import { Button } from '@liulidesign/core/Button';
import { Dialog } from '@liulidesign/core/Dialog';
import { Lightbox } from '@liulidesign/core/Lightbox';
import { DropdownMenu, type DropdownMenuEntry } from '@liulidesign/core/DropdownMenu';
import { IconButton } from '@liulidesign/core/IconButton';
import { Popover } from '@liulidesign/core/Popover';
import { ProgressBar } from '@liulidesign/core/ProgressBar';
import {
  callout,
  codeBlock,
  columns,
  details,
  divider,
  extractOutline,
  heading,
  list,
  listItem,
  mathBlock,
  newBlockId,
  paragraph,
  plainText,
  quote,
  table,
} from '../schema/helpers';
import { normalizeInlines } from '../schema/validate';
import type {
  AnyJinshuOp,
  BlockInput,
  ConflictItem,
  FilePolicy,
  HeadingLevel,
  HistoryEntry,
  JinshuBlock,
  JinshuDocument,
  MediaPolicy,
  NoteWord,
  OpBatch,
  Origin,
  Suggestion,
} from '../schema/types';
import type { AgentActivity, AgentBatch, ChannelClientMsg, ChannelServerMsg } from '../source/types';
import { blockTopSpacing, renderBlock, type BlockRenderContext, type CustomBlockRenderer } from '../internal/Blocks';
import { renderInlineRuns, type InlineAtomRenderer } from '../internal/InlineRuns';
import { useJinshuChannelContext, type JinshuChannelAction } from '../internal/channel';
import { MathFormulaView } from '../internal/MathView';
import type { JinshuNavHandle } from '../internal/nav';
import { jinshuViewRecipe as r } from '../JinshuView/JinshuView.recipe';
import { jinshuText } from '../i18n/text';
import {
  CalloutBlockEditor,
  CodeBlockEditor,
  ColumnsBlockEditor,
  DetailsBlockEditor,
  ImageBlockInspector,
  ListBlockEditor,
  MediaBlockInspector,
  QuoteBlockEditor,
  TableBlockEditor,
  type BlockEditorCallbacks,
} from './BlockEditors';
import { editorIcons } from './editorIcons';
import { jinshuEditorRecipe as e } from './JinshuEditor.recipe';
import {
  hasTrailingShorthandMark,
  atomsIn,
  inlinesToShorthand,
  shorthandToInlines,
  toggleShorthandWrap,
} from './inlineMarkdown';

export { JinshuProvider, type JinshuChannelAction, type JinshuProviderProps } from '../internal/channel';
export type { CustomBlockRenderer } from '../internal/Blocks';
export type { InlineAtomRenderer } from '../internal/InlineRuns';
export type { JinshuNavHandle } from '../internal/nav';

export interface JinshuEditorProps {
  /** The page's `syncDoc` action, which Logic answers with `source.serve`: edits, history, saving, the review queue and agent changes all go through the source. */
  readonly channel?: JinshuChannelAction;
  /** Which document `channel` serves. */
  readonly docKey?: string;
  /** The document, held by the page, when there is no `channel`; each edit comes back through `onOps`. */
  readonly doc?: JinshuDocument;
  /** The ops of each edit to `doc`, for the page to apply with `applyOps`. */
  readonly onOps?: (ops: readonly AnyJinshuOp[]) => void;
  /** Read-only: hides editing chrome, keeps selection and copy. */
  readonly isReadOnly?: boolean;
  /** Pins the single-row icon formatting ribbon at the top of the scroll container. @default true */
  readonly isToolbarSticky?: boolean;
  /** Extra quiet action(s) docked on the trailing side of the sticky ribbon. */
  readonly toolbarActions?: Child;
  /** Hands blocks to the app's agent as context; offered in each block's menu when set. */
  readonly onAskAgent?: (blockIds: readonly string[]) => void;
  /** Enable `useLeaveGuard` when draft has unsaved changes. @default true */
  readonly hasLeaveGuard?: boolean;
  /** Unsaved changes, for the leave guard and the toolbar's Save; falls back to `saveStatus === 'dirty'`. */
  readonly isDirty?: boolean;
  /** Where saving stands, as the page hears it from Logic; on a live `channel` the toolbar shows it, with Save or Retry. */
  readonly saveStatus?: 'saved' | 'dirty' | 'saving' | 'failed';
  /** Shared with a `JinshuOutline` from `useJinshuNav()`. */
  readonly nav?: JinshuNavHandle;
  /** A top-level block to open at, once: where an agent's change landed (a tool result's `page`). */
  readonly openAt?: string;
  /** As on JinshuView. */
  readonly filePolicy?: FilePolicy;
  /** As on JinshuView. */
  readonly mediaPolicy?: MediaPolicy;
  /** As on JinshuView. */
  readonly resolveSrc?: (src: string) => string;
  /** Renderers for the app's own block types, keyed by `x-scope-name`; they edit through their attrs. */
  readonly customBlocks?: Readonly<Record<string, CustomBlockRenderer>>;
  /** Renderers for the app's own inline atoms, keyed by `x-scope-name`. */
  readonly customInlines?: Readonly<Record<string, InlineAtomRenderer>>;
  /** As on JinshuView. */
  readonly onLinkPress?: (info: { readonly blockId: string; readonly href: string }) => void;
  /** As on JinshuView. */
  readonly onFileAction?: (info: { readonly blockId: string; readonly src: string; readonly action: 'download' }) => void;
  readonly className?: string;
  readonly 'data-testid'?: string;
}

interface EditorLiveState {
  readonly doc: JinshuDocument | null;
  readonly revision: number;
  readonly suggestions: readonly Suggestion[];
  readonly conflicts: readonly ConflictItem[];
  readonly history: readonly HistoryEntry[];
  readonly canUndo: boolean;
  readonly canRedo: boolean;
  readonly filePolicy?: FilePolicy;
  readonly hasMedia?: boolean;
  /** Upload progress by block id, 0–1, while a picked file uploads. */
  readonly uploads?: Readonly<Record<string, number>>;
  readonly saveRequest?: { readonly origin: Origin; readonly summary?: string } | null;
  readonly activities?: readonly AgentActivity[];
  /** Agent batches the person has not dismissed or undone yet. */
  readonly agentBatches?: readonly AgentBatch[];
  readonly streamPreview?: {
    readonly streamId: string;
    readonly origin: Origin;
    readonly blocks: readonly JinshuBlock[];
    readonly status: 'streaming' | 'done' | 'aborted';
  } | null;
}

interface SlashItem {
  readonly id: string;
  readonly keywords: string;
  readonly badge: string;
  /** Inserted through the app's `media.pick`; offered only when the source has one. */
  readonly media?: MediaKind;
}

type MediaKind = 'image' | 'video' | 'audio' | 'file';

const SLASH_ITEMS: readonly SlashItem[] = [
  { id: 'paragraph', keywords: 'p paragraph text 段落 正文 zw', badge: 'T' },
  { id: 'h1', keywords: 'h1 heading title 标题 一级 bt', badge: 'H1' },
  { id: 'h2', keywords: 'h2 heading 标题 二级 bt', badge: 'H2' },
  { id: 'h3', keywords: 'h3 heading 标题 三级 bt', badge: 'H3' },
  { id: 'bullet', keywords: 'ul bullet list 列表 无序 lb', badge: '•' },
  { id: 'ordered', keywords: 'ol ordered list 列表 有序 lb', badge: '1.' },
  { id: 'task', keywords: 'todo task check 任务 待办 rw', badge: '☑' },
  { id: 'quote', keywords: 'quote blockquote 引用 yy', badge: '“' },
  { id: 'callout', keywords: 'callout tip info 提示 高亮 ts', badge: '!' },
  { id: 'details', keywords: 'details fold toggle 折叠 zd', badge: '▸' },
  { id: 'image', keywords: 'image img picture photo 图片 照片 tp', badge: '▣', media: 'image' },
  { id: 'video', keywords: 'video movie 视频 sp', badge: '▶', media: 'video' },
  { id: 'audio', keywords: 'audio sound voice 音频 语音 yp', badge: '♪', media: 'audio' },
  { id: 'file', keywords: 'file attachment pdf 附件 文件 fj', badge: '⎘', media: 'file' },
  { id: 'table', keywords: 'table grid 表格 bg', badge: '⊞' },
  { id: 'code', keywords: 'code ts js 代码 dm', badge: '</>' },
  { id: 'math', keywords: 'math latex formula 公式 gs', badge: '∑' },
  { id: 'columns', keywords: 'columns grid 双栏 分栏 fl', badge: '▥' },
  { id: 'divider', keywords: 'hr divider line 分割线 fgx', badge: '—' },
];

/** Every type a text block can turn into, in one place for the format bar and the block handle. */
const TURN_INTO: readonly { readonly id: string; readonly badge: string }[] = [
  { id: 'paragraph', badge: 'T' },
  { id: 'h1', badge: 'H1' },
  { id: 'h2', badge: 'H2' },
  { id: 'h3', badge: 'H3' },
  { id: 'h4', badge: 'H4' },
  { id: 'bullet', badge: '•' },
  { id: 'ordered', badge: '1.' },
  { id: 'task', badge: '☑' },
  { id: 'quote', badge: '“' },
  { id: 'callout', badge: '!' },
  { id: 'code', badge: '</>' },
];

/** A block type's name, as the slash menu, insert panel and turn-into menu show it. During render only. */
function blockTypeLabel(id: string): string {
  if (/^h\d$/.test(id)) return jinshuText('@jinshu.editor.blockType.heading', { level: Number(id.slice(1)) });
  return jinshuText(`@jinshu.editor.blockType.${id}`);
}

/** A history note the reader words in their own language. */
function note(key: NoteWord['key'], args?: NoteWord['args']): NoteWord {
  return args ? { key, args } : { key };
}

/** The kind a block reads as in a note: its type, its list style, or a heading with its level. */
function kindOf(block: BlockInput | JinshuBlock): NoteWord['args'] {
  if (block.type === 'heading') return { kind: 'heading', level: block.level };
  if (block.type === 'list') return { kind: block.style };
  return { kind: block.type };
}

function turnIntoOp(block: JinshuBlock, kind: string): AnyJinshuOp | null {
  switch (kind) {
    case 'paragraph': return { op: 'setType', id: block.id, type: 'paragraph', noteWord: note('@jinshu.note.turnInto', { kind }) };
    case 'h1': case 'h2': case 'h3': case 'h4': {
      const level = Number(kind.slice(1)) as HeadingLevel;
      return { op: 'setType', id: block.id, type: 'heading', level, noteWord: note('@jinshu.note.turnInto', { kind: 'heading', level }) };
    }
    case 'bullet': case 'ordered': case 'task':
      if (block.type === 'list') return { op: 'update', id: block.id, attrs: { style: kind }, noteWord: note('@jinshu.note.listStyle') };
      return { op: 'setType', id: block.id, type: 'list', style: kind, noteWord: note('@jinshu.note.turnInto', { kind }) };
    case 'quote': return { op: 'setType', id: block.id, type: 'quote', noteWord: note('@jinshu.note.turnInto', { kind }) };
    case 'callout': return { op: 'setType', id: block.id, type: 'callout', tone: 'info', noteWord: note('@jinshu.note.turnInto', { kind }) };
    case 'code': return { op: 'setType', id: block.id, type: 'code', language: 'text', noteWord: note('@jinshu.note.turnInto', { kind }) };
    default: return null;
  }
}

function currentTurnInto(block: JinshuBlock | null): string | null {
  if (!block) return null;
  if (block.type === 'heading') return `h${block.level}`;
  if (block.type === 'list') return block.style;
  if (block.type === 'paragraph' || block.type === 'quote' || block.type === 'callout' || block.type === 'code') return block.type;
  return null;
}

const SAVED_PATH = [
  { d: 'M3.5 8.5l3 3 6-7', fill: 'none', stroke: 'currentColor', strokeWidth: 1.75, strokeLinecap: 'round', strokeLinejoin: 'round' },
] as const;

// No label: the person editing is named in their own language when drawn.
const HUMAN_ORIGIN: Origin = { kind: 'human', id: 'editor' };

/** Who made a change, as the history and review show it. During render only. */
function originName(origin: Origin): string {
  const name = origin.label ?? (origin.kind === 'human' ? jinshuText('@jinshu.editor.origin.you') : origin.id);
  return origin.via ? jinshuText('@jinshu.editor.origin.via', { name, via: origin.via }) : name;
}

/** A history or suggestion note: the editor's word, else the writer's text. During render only. */
function noteText(item: { readonly note?: string; readonly noteWord?: NoteWord }): string | undefined {
  if (item.noteWord) {
    try {
      return jinshuText(item.noteWord.key, item.noteWord.args);
    } catch {
      // A note from a newer editor whose word this build lacks falls back to its text.
    }
  }
  return item.note;
}

/** The top-level block that holds `id`, for marks and jumps on nested changes. */
function topLevelOf(id: string, doc: JinshuDocument): string | null {
  if (doc.blocks.some(b => b.id === id)) return id;
  return doc.blocks.find(b => JSON.stringify(b).includes(`"id":"${id}"`))?.id ?? null;
}

/** Blocks an unselected tap selects on a coarse pointer, instead of reaching their own controls. */
function isObjectBlock(block: JinshuBlock): boolean {
  return block.type === 'image' || block.type === 'video' || block.type === 'audio' || block.type === 'file' || block.type === 'divider' || block.type === 'table' || block.type.startsWith('x-');
}

type MediaBlockKind = 'image' | 'video' | 'audio' | 'file';
const isMediaBlock = (block: JinshuBlock): block is Extract<JinshuBlock, { type: MediaBlockKind }> =>
  block.type === 'image' || block.type === 'video' || block.type === 'audio' || block.type === 'file';

function isInlineTextBlock(block: JinshuBlock): boolean {
  return block.type === 'paragraph' || block.type === 'heading' || block.type === 'math';
}

function shorthandOf(block: JinshuBlock): string {
  if (block.type === 'paragraph' || block.type === 'heading') return inlinesToShorthand(block.runs);
  if (block.type === 'math') return block.latex;
  return '';
}

/** The starter text a new block is written with, in the page's language. During render only. */
function blockSeeds() {
  return {
    listItem: jinshuText('@jinshu.editor.seed.listItem'),
    firstItem: jinshuText('@jinshu.editor.seed.firstItem'),
    task: jinshuText('@jinshu.editor.seed.task'),
    quote: jinshuText('@jinshu.editor.seed.quote'),
    calloutTitle: jinshuText('@jinshu.editor.seed.calloutTitle'),
    calloutBody: jinshuText('@jinshu.editor.seed.calloutBody'),
    calloutText: jinshuText('@jinshu.editor.seed.calloutText'),
    detailsSummary: jinshuText('@jinshu.editor.seed.detailsSummary'),
    detailsBody: jinshuText('@jinshu.editor.seed.detailsBody'),
    columnStart: jinshuText('@jinshu.editor.seed.columnStart'),
    columnEnd: jinshuText('@jinshu.editor.seed.columnEnd'),
    linkText: jinshuText('@jinshu.editor.seed.linkText'),
    text: jinshuText('@jinshu.editor.seed.text'),
    // Up to the table picker's six columns.
    columns: [1, 2, 3, 4, 5, 6].map(column => jinshuText('@jinshu.editor.seed.column', { column })),
  };
}
type BlockSeeds = ReturnType<typeof blockSeeds>;

function makeBlockForSlash(kind: string, seed: BlockSeeds, rows = 2, cols = 3): BlockInput {
  switch (kind) {
    case 'h1': return heading(1, '');
    case 'h2': return heading(2, '');
    case 'h3': return heading(3, '');
    case 'h4': return heading(4, '');
    case 'bullet': return list('bullet', [listItem(seed.listItem)]);
    case 'ordered': return list('ordered', [listItem(seed.firstItem)]);
    case 'task': return list('task', [listItem(seed.task, { checked: false })]);
    case 'quote': return quote([paragraph(seed.quote)]);
    case 'callout': return callout('info', [paragraph(seed.calloutBody)], { title: seed.calloutTitle });
    case 'details': return details(seed.detailsSummary, [paragraph(seed.detailsBody)], { defaultOpen: true });
    case 'columns': return columns([[paragraph(seed.columnStart)], [paragraph(seed.columnEnd)]]);
    case 'code': return codeBlock('ts', '// code');
    case 'math': return mathBlock('\\frac{a + b}{2} \\ge \\sqrt{ab}');
    case 'table':
      return table(
        Array.from({ length: cols }, (_, c) => seed.columns[c]),
        Array.from({ length: rows }, () => Array.from({ length: cols }, () => '')),
      );
    case 'divider': return divider();
    default: return paragraph('');
  }
}

/** Clones a block with fresh IDs for Duplicate. */
function duplicateBlockInput(block: JinshuBlock): BlockInput {
  const copy = JSON.parse(JSON.stringify(block)) as Record<string, unknown>;
  delete copy.id;
  if (Array.isArray(copy.blocks)) {
    copy.blocks = (copy.blocks as JinshuBlock[]).map(duplicateBlockInput);
  }
  if (Array.isArray(copy.items)) {
    copy.items = (copy.items as { id?: string; blocks?: JinshuBlock[] }[]).map(it => {
      const next = { ...it };
      delete next.id;
      if (next.blocks) next.blocks = next.blocks.map(duplicateBlockInput) as unknown as JinshuBlock[];
      return next;
    });
  }
  if (Array.isArray(copy.rows)) {
    copy.rows = (copy.rows as { id?: string }[]).map(({ id: _id, ...row }) => row);
  }
  if (Array.isArray(copy.columns)) {
    copy.columns = (copy.columns as { id?: string; blocks: JinshuBlock[] }[]).map(col => ({
      blocks: col.blocks.map(duplicateBlockInput),
    }));
  }
  return copy as unknown as BlockInput;
}

function suggestionSummary(sug: Suggestion): string {
  if (sug.kind === 'range') {
    return `→ "${plainText(sug.insert)}"`;
  }
  return sug.ops.map(o => o.op).join(' · ');
}

function suggestionAnchor(sug: Suggestion, doc: JinshuDocument): string | null {
  const topIds = new Set(doc.blocks.map(b => b.id));
  const candidates: string[] = [];
  if (sug.kind === 'range') candidates.push(sug.block);
  else {
    for (const op of sug.ops) {
      if ('id' in op && typeof op.id === 'string') candidates.push(op.id);
      else if (op.op === 'insert') {
        if ('after' in op.at) candidates.push(op.at.after);
        else if ('before' in op.at) candidates.push(op.at.before);
      }
    }
  }
  for (const id of candidates) {
    if (topIds.has(id)) return id;
    const owner = doc.blocks.find(b => JSON.stringify(b).includes(`"id":"${id}"`));
    if (owner) return owner.id;
  }
  return null;
}

function blockPlainText(block: JinshuBlock | undefined): string {
  if (!block) return '';
  if (block.type === 'paragraph' || block.type === 'heading') return plainText(block.runs);
  if (block.type === 'code') return block.code;
  if (block.type === 'math') return block.latex;
  return '';
}

function ToolbarSep() {
  return <view aria-hidden className={e.toolbarSep.base} />;
}

/** Crisp 32×32 typographic mark button matching icon buttons in the sticky ribbon. */
function MarkGlyphButton({
  glyph,
  label,
  active,
  disabled,
  textClass,
  onClick,
}: {
  readonly glyph: string;
  readonly label: string;
  readonly active?: boolean;
  readonly disabled?: boolean;
  readonly textClass?: string;
  readonly onClick: () => void;
}) {
  return (
    <button
      aria-label={label}
      aria-pressed={active || undefined}
      disabled={disabled}
      onClick={onClick}
      className={cx(
        e.markButton.base,
        disabled ? e.markButton.disabled : active ? e.markButton.active : e.markButton.idle,
      )}
    >
      <text className={cx(e.markGlyph.base, active ? e.markGlyph.active : disabled ? e.markGlyph.disabled : e.markGlyph.idle, textClass)}>
        {glyph}
      </text>
    </button>
  );
}

function PaletteButton({ badge, label, onClick }: { readonly badge: string; readonly label: string; readonly onClick: () => void }) {
  return (
    <button onClick={onClick} className={e.insertPanel.item}>
      <text aria-hidden className={e.insertPanel.badge}>{badge}</text>
      <text className={e.insertPanel.label}>{label}</text>
    </button>
  );
}

/** Insert palette: blocks, media through the app's picker, and a table sized on a grid. */
function InsertBlockPanel({
  nameKey,
  hasMedia,
  onPickBlock,
  onPickMedia,
  onPickTable,
}: {
  readonly nameKey: string;
  readonly hasMedia: boolean;
  readonly onPickBlock: (kind: string) => void;
  readonly onPickMedia: (kind: MediaKind) => void;
  readonly onPickTable: (rows: number, cols: number) => void;
}) {
  const hoverGrid = useLocal<{ readonly r: number; readonly c: number }>(`jinshu-tbl-pick-${nameKey}`, { r: 2, c: 3 });
  const gr = hoverGrid.value.r;
  const gc = hoverGrid.value.c;
  const blocks = SLASH_ITEMS.filter(item => !item.media && item.id !== 'table' && item.id !== 'paragraph');
  const media = SLASH_ITEMS.filter(item => item.media);

  return (
    <view className={e.insertPanel.base}>
      <view className={e.insertPanel.section}>
        <text className={e.insertPanel.heading}>{jinshuText('@jinshu.editor.insert.blocks')}</text>
        <view className={e.insertPanel.grid}>
          {blocks.map(item => <PaletteButton key={item.id} badge={item.badge} label={blockTypeLabel(item.id)} onClick={() => onPickBlock(item.id)} />)}
        </view>
      </view>
      {hasMedia && (
        <view className={e.insertPanel.section}>
          <text className={e.insertPanel.heading}>{jinshuText('@jinshu.editor.insert.media')}</text>
          <view className={e.insertPanel.grid}>
            {media.map(item => <PaletteButton key={item.id} badge={item.badge} label={blockTypeLabel(item.id)} onClick={() => onPickMedia(item.media!)} />)}
          </view>
        </view>
      )}
      <view className={e.tablePicker.base}>
        <view className={e.tablePicker.header}>
          <text className={e.tablePicker.title}>{jinshuText('@jinshu.editor.blockType.table')}</text>
          <text className={e.tablePicker.size}>{`${gr} × ${gc}`}</text>
        </view>
        <view className={e.tablePicker.grid}>
          {[1, 2, 3, 4].map(rIdx => (
            <view key={`gr-${rIdx}`} className={e.tablePicker.row}>
              {[1, 2, 3, 4, 5, 6].map(cIdx => {
                const lit = rIdx <= gr && cIdx <= gc;
                return (
                  <button
                    key={`gc-${rIdx}-${cIdx}`}
                    aria-label={jinshuText('@jinshu.editor.insert.tableSize', { rows: rIdx, columns: cIdx })}
                    onPointerEnter={() => hoverGrid.set({ r: rIdx, c: cIdx })}
                    onClick={() => onPickTable(rIdx, cIdx)}
                    className={cx(e.tablePicker.cell, lit ? e.tablePicker.cellLit : e.tablePicker.cellIdle)}
                  />
                );
              })}
            </view>
          ))}
        </view>
      </view>
    </view>
  );
}

/**
 * Publication-grade block editor & AI co-authoring workspace:
 * - Sticky single-row icon formatting ribbon (`isToolbarSticky`), zero layout shift on block focus.
 * - Floating `<Popover>` / `<DropdownMenu>` for block handles (`+` / `⋮`), Link insertion, Formula live preview, History, and `+ Add` Table Grid Picker.
 * - Direct in-place editing with auto-growing `<textarea maxRows>`, `quiet-md`-style table edge bars & danger preview, and in-place AI suggestion diff review.
 */
export function JinshuEditor(props: JinshuEditorProps) {
  const { env } = useView();
  const ownScope = useId();
  // Driven by a nav, the editor takes its scope so the nav's `scrollTo` finds the blocks.
  const scope = props.nav ? props.nav.scope : ownScope;
  const ctxChannel = useJinshuChannelContext();
  const activeChannel = props.channel ?? ctxChannel;
  const docKey = props.docKey ?? 'default';

  const live = useLocal<any>(`jinshu-ed-live-${docKey}`, {
    doc: null,
    revision: 1,
    suggestions: [],
    conflicts: [],
    history: [],
    canUndo: false,
    canRedo: false,
    saveRequest: null,
    activities: [],
    agentBatches: [],
    streamPreview: null,
  } as EditorLiveState);

  const seed = blockSeeds();
  const editingTitle = useLocal<boolean>(`jinshu-ed-title-ed-${docKey}`, false);
  const titleDraft = useLocal<string>(`jinshu-ed-title-val-${docKey}`, '');
  const activeId = useLocal<string | null>(`jinshu-ed-active-${docKey}`, null);
  const draftText = useLocal<string>(`jinshu-ed-draft-${docKey}`, '');
  const hoveredId = useLocal<string | null>(`jinshu-ed-hover-${docKey}`, null);
  const openedAt = useLocal(`jinshu-ed-opened-at-${docKey}`, '');
  const selectedNonTextId = useLocal<string | null>(`jinshu-ed-sel-nt-${docKey}`, null);
  const slashAfterId = useLocal<string | null>(`jinshu-ed-slash-${docKey}`, null);
  const slashQuery = useLocal<string>(`jinshu-ed-slash-q-${docKey}`, '');
  const slashIndex = useLocal<number>(`jinshu-ed-slash-idx-${docKey}`, 0);
  const showFind = useLocal<boolean>(`jinshu-ed-find-open-${docKey}`, false);
  const findQuery = useLocal<string>(`jinshu-ed-find-q-${docKey}`, '');
  const replaceQuery = useLocal<string>(`jinshu-ed-repl-q-${docKey}`, '');
  const queueCursor = useLocal<number>(`jinshu-ed-queue-cur-${docKey}`, 0);
  const changeCursor = useLocal<number>(`jinshu-ed-change-cur-${docKey}`, 0);
  // The selected block shows its inspector only after "properties".
  const inspecting = useLocal<boolean>(`jinshu-ed-inspect-${docKey}`, false);
  const previewId = useLocal<string | null>(`jinshu-ed-preview-${docKey}`, null);

  // Link & Equation popover state (triggers own open/close via native popoverTarget; incrementing epoch closes after insert)
  const popEpoch = useLocal<number>(`jinshu-ed-pop-ep-${docKey}`, 0);
  const linkLabel = useLocal<string>(`jinshu-ed-link-lbl-${docKey}`, '');
  const linkHref = useLocal<string>(`jinshu-ed-link-href-${docKey}`, 'https://');
  const mathDraft = useLocal<string>(`jinshu-ed-math-val-${docKey}`, '\\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a}');
  const gutterAddId = useLocal<string | null>(`jinshu-ed-gadd-${docKey}`, null);

  const chan = activeChannel
    ? useChannel(`jinshu-editor-${docKey}`, activeChannel, {
        params: { doc: docKey },
        onData(msg: ChannelServerMsg) {
          const cur = live.value as EditorLiveState;
          if (msg.t === 'snapshot') {
            live.set({
              ...cur,
              doc: msg.doc as JinshuDocument,
              revision: msg.revision,
              filePolicy: msg.policy?.filePolicy ?? cur.filePolicy,
              hasMedia: msg.policy?.hasMedia ?? cur.hasMedia,
            });
          } else if (msg.t === 'suggestions') {
            live.set({ ...cur, suggestions: msg.suggestions as Suggestion[] });
          } else if (msg.t === 'conflicts') {
            live.set({ ...cur, conflicts: msg.conflicts as ConflictItem[] });
          } else if (msg.t === 'history') {
            const entries = msg.entries as HistoryEntry[];
            const live_ = new Set(entries.filter(h => !h.undone).map(h => h.batchId));
            live.set({
              ...cur,
              history: entries,
              canUndo: msg.canUndo,
              canRedo: msg.canRedo,
              // A batch undone from anywhere drops its bar and marks.
              agentBatches: (cur.agentBatches ?? []).filter(b => live_.has(b.batchId)),
            });
          } else if (msg.t === 'saveRequest') {
            live.set({ ...cur, saveRequest: { origin: msg.origin, ...(msg.summary ? { summary: msg.summary } : {}) } });
          } else if (msg.t === 'activity') {
            live.set({ ...cur, activities: msg.activities as AgentActivity[] });
          } else if (msg.t === 'agentBatches') {
            live.set({ ...cur, agentBatches: [...(cur.agentBatches ?? []), ...(msg.batches as AgentBatch[])] });
          } else if (msg.t === 'stream') {
            live.set({
              ...cur,
              streamPreview:
                msg.status === 'streaming'
                  ? { streamId: msg.streamId, origin: msg.origin as Origin, blocks: msg.blocks as JinshuBlock[], status: msg.status }
                  : null,
            });
          } else if (msg.t === 'uploadProgress') {
            const uploads = { ...(cur.uploads ?? {}) };
            if (msg.ratio >= 1) delete uploads[msg.blockId];
            else uploads[msg.blockId] = msg.ratio;
            live.set({ ...cur, uploads });
          } else if (msg.t === 'reject' && msg.snapshot) {
            live.set({
              ...cur,
              doc: msg.snapshot as JinshuDocument,
              revision: msg.revision ?? cur.revision,
            });
          }
        },
      })
    : null;

  const isDirty = props.isDirty ?? props.saveStatus === 'dirty';
  const guard = useLeaveGuard(`jinshu-draft-${docKey}`, {
    enabled: !props.isReadOnly && props.hasLeaveGuard !== false && isDirty,
  });

  const liveState = live.value as EditorLiveState;
  const doc = activeChannel ? liveState.doc : (props.doc ?? null);
  const suggestions = liveState.suggestions;
  const conflicts = liveState.conflicts;
  const history = liveState.history;
  const saveRequest = liveState.saveRequest;
  const streamPreview = liveState.streamPreview;
  const activities = liveState.activities ?? [];
  const agentBatches = liveState.agentBatches ?? [];
  const readOnly = props.isReadOnly === true;
  const stickyToolbar = props.isToolbarSticky !== false;
  const isDesktop = env.pointer === 'fine' && env.sizeClass === 'regular';
  const isCoarse = env.pointer === 'coarse';

  const sendMsg = (msg: ChannelClientMsg) => {
    chan?.send(msg);
  };

  const dispatchOps = (ops: readonly AnyJinshuOp[]) => {
    if (readOnly || ops.length === 0) return;
    if (chan) {
      const batch: OpBatch = {
        batchId: newBlockId('ed'),
        origin: HUMAN_ORIGIN,
        base: liveState.revision,
        ops,
        mode: 'direct',
      };
      sendMsg({ t: 'ops', batch });
    } else {
      props.onOps?.(ops);
    }
  };

  const commitActiveDraft = (block: JinshuBlock, nextValue: string) => {
    const prevValue = shorthandOf(block);
    if (nextValue === prevValue) return;
    if (block.type === 'paragraph' || block.type === 'heading') {
      const runs = shorthandToInlines(nextValue, { inHeading: block.type === 'heading', existingAtoms: atomsIn(block.runs) });
      // A draft that reads back to the same runs (e.g. only escapes differ) is no edit.
      if (JSON.stringify(normalizeInlines(runs)) === JSON.stringify(normalizeInlines(block.runs))) return;
      dispatchOps([
        {
          op: 'replace',
          id: block.id,
          blocks: [
            block.type === 'heading'
              ? { id: block.id, type: 'heading', level: block.level, runs }
              : { id: block.id, type: 'paragraph', runs },
          ],
        },
      ]);
    } else if (block.type === 'math') {
      dispatchOps([{ op: 'update', id: block.id, attrs: { latex: nextValue } }]);
    }
  };

  /** One selected block at a time; selecting closes the inspector and ends text editing. */
  const selectBlock = (blockId: string | null) => {
    if (blockId !== selectedNonTextId.value) inspecting.set(false);
    selectedNonTextId.set(blockId);
  };

  const focusBlock = (block: JinshuBlock) => {
    if (readOnly) return;
    if (activeId.value && doc && activeId.value !== block.id) {
      const prev = doc.blocks.find(b => b.id === activeId.value);
      if (prev && isInlineTextBlock(prev)) commitActiveDraft(prev, draftText.value);
    }
    slashAfterId.set(null);
    slashQuery.set('');
    slashIndex.set(0);
    selectBlock(null);
    activeId.set(block.id);
    draftText.set(shorthandOf(block));
    sendMsg({ t: 'focus', blockIds: [block.id] });
  };

  const blurActive = (block: JinshuBlock) => {
    commitActiveDraft(block, draftText.value);
    if (activeId.value === block.id) {
      activeId.set(null);
      sendMsg({ t: 'focus', blockIds: [] });
    }
  };

  // Commit on blur but keep the block active: a tap on the format bar blurs the field first.
  const commitOnBlur = (block: JinshuBlock) => commitActiveDraft(block, draftText.value);

  const pickMedia = (kind: MediaKind, afterId: string | null, replacingId?: string) => {
    if (readOnly) return;
    sendMsg({ t: 'media', action: 'pick', kind, at: afterId ? { after: afterId } : { end: 'doc' }, ...(replacingId ? { replacingId } : {}) });
  };

  const handleLiveInput = (block: JinshuBlock, next: string) => {
    if (block.type === 'paragraph' && next.startsWith('/')) {
      draftText.set(next);
      slashAfterId.set(block.id);
      slashQuery.set(next.slice(1).trim());
      slashIndex.set(0);
      return;
    }
    if (slashAfterId.value === block.id) {
      slashAfterId.set(null);
      slashQuery.set('');
      slashIndex.set(0);
    }

    if (block.type === 'paragraph') {
      const hMatch = /^(#{1,4}) (.*)$/.exec(next);
      if (hMatch) {
        const level = hMatch[1].length as HeadingLevel;
        const rest = hMatch[2];
        dispatchOps([
          { op: 'replace', id: block.id, blocks: [{ id: block.id, type: 'heading', level, runs: shorthandToInlines(rest, { inHeading: true, existingAtoms: atomsIn(block.runs) }) }], noteWord: note('@jinshu.note.turnInto', { kind: 'heading', level }) },
        ]);
        draftText.set(rest);
        return;
      }
      if (next.startsWith('- [ ] ') || next.startsWith('[] ')) {
        const rest = next.replace(/^(?:- \[ \] |\[\] )/, '');
        dispatchOps([{ op: 'replace', id: block.id, blocks: [list('task', [listItem(rest || seed.task, { checked: false })], { id: block.id })], noteWord: note('@jinshu.note.turnInto', { kind: 'task' }) }]);
        activeId.set(null);
        return;
      }
      if (next.startsWith('- ') || next.startsWith('* ')) {
        const rest = next.slice(2);
        dispatchOps([{ op: 'replace', id: block.id, blocks: [list('bullet', [listItem(rest || seed.listItem)], { id: block.id })], noteWord: note('@jinshu.note.turnInto', { kind: 'bullet' }) }]);
        activeId.set(null);
        return;
      }
      if (next.startsWith('1. ')) {
        const rest = next.slice(3);
        dispatchOps([{ op: 'replace', id: block.id, blocks: [list('ordered', [listItem(rest || seed.firstItem)], { id: block.id })], noteWord: note('@jinshu.note.turnInto', { kind: 'ordered' }) }]);
        activeId.set(null);
        return;
      }
      if (next.startsWith('!> ')) {
        const rest = next.slice(3);
        dispatchOps([{ op: 'replace', id: block.id, blocks: [callout('info', [paragraph(rest || seed.calloutText)], { id: block.id, title: seed.calloutTitle })], noteWord: note('@jinshu.note.turnInto', { kind: 'callout' }) }]);
        activeId.set(null);
        return;
      }
      if (next.startsWith('> ')) {
        const rest = next.slice(2);
        dispatchOps([{ op: 'replace', id: block.id, blocks: [quote([paragraph(rest || seed.quote)], block.id)], noteWord: note('@jinshu.note.turnInto', { kind: 'quote' }) }]);
        activeId.set(null);
        return;
      }
      if (next === '---') {
        const nextP = paragraph('');
        dispatchOps([{ op: 'replace', id: block.id, blocks: [divider(block.id), nextP], noteWord: note('@jinshu.note.insert', { kind: 'divider' }) }]);
        activeId.set(nextP.id);
        draftText.set('');
        return;
      }
      if (next.startsWith('```')) {
        const lang = next.slice(3).trim() || 'ts';
        dispatchOps([{ op: 'replace', id: block.id, blocks: [codeBlock(lang, '', { id: block.id })], noteWord: note('@jinshu.note.turnInto', { kind: 'code' }) }]);
        activeId.set(null);
        return;
      }
    }

    draftText.set(next);
  };

  const insertBlockAt = (afterBlockId: string | null, blk: BlockInput, replaceTarget = false) => {
    if (replaceTarget && afterBlockId) {
      dispatchOps([{ op: 'replace', id: afterBlockId, blocks: [blk], noteWord: note('@jinshu.note.turnInto', kindOf(blk)) }]);
      if (blk.type === 'paragraph' || blk.type === 'heading' || blk.type === 'math') {
        activeId.set(afterBlockId);
        draftText.set(blk.type === 'math' ? blk.latex : '');
      } else {
        activeId.set(null);
      }
    } else {
      const newId = newBlockId('b');
      const pos = afterBlockId && doc?.blocks.some(b => b.id === afterBlockId) ? { after: afterBlockId } : ({ end: 'doc' } as const);
      dispatchOps([{ op: 'insert', at: pos, blocks: [{ ...blk, id: newId }], noteWord: note('@jinshu.note.insert', kindOf(blk)) }]);
      if (blk.type === 'paragraph' || blk.type === 'heading' || blk.type === 'math') {
        activeId.set(newId);
        draftText.set(blk.type === 'math' ? blk.latex : '');
      }
    }
    slashAfterId.set(null);
    slashQuery.set('');
    slashIndex.set(0);
  };

  const applySlashChoice = (targetBlockId: string, kind: string, replaceEmpty: boolean) => {
    const media = SLASH_ITEMS.find(item => item.id === kind)?.media;
    if (media) {
      // The typed `/query` paragraph becomes the picked media; a cancelled pick leaves it empty.
      if (replaceEmpty) draftText.set('');
      pickMedia(media, targetBlockId, replaceEmpty ? targetBlockId : undefined);
      slashAfterId.set(null);
      slashQuery.set('');
      slashIndex.set(0);
      activeId.set(null);
      return;
    }
    insertBlockAt(targetBlockId, makeBlockForSlash(kind, seed), replaceEmpty);
  };

  const applyFormatButton = (prefix: string, suffix = prefix) => {
    if (!doc || !activeId.value) return;
    const blk = doc.blocks.find(b => b.id === activeId.value);
    if (!blk || (blk.type !== 'paragraph' && blk.type !== 'heading')) return;
    const next = toggleShorthandWrap(draftText.value, prefix, suffix, { placeholder: prefix === '[' ? seed.linkText : seed.text });
    draftText.set(next);
    commitActiveDraft(blk, next);
  };

  const insertInlineSnippet = (snippet: string) => {
    if (!doc) return;
    const blk = activeId.value ? doc.blocks.find(b => b.id === activeId.value) : undefined;
    if (blk && (blk.type === 'paragraph' || blk.type === 'heading')) {
      const sep = draftText.value && !/\s$/.test(draftText.value) ? ' ' : '';
      const next = `${draftText.value}${sep}${snippet}`;
      draftText.set(next);
      commitActiveDraft(blk, next);
      return;
    }
    const nextP = paragraph(shorthandToInlines(snippet));
    dispatchOps([{ op: 'insert', at: { end: 'doc' }, blocks: [nextP], noteWord: note('@jinshu.note.insertText') }]);
  };

  const blockCb: BlockEditorCallbacks = {
    testScope: props['data-testid'],
    dispatchOps,
    onFocusBlock(blockId) {
      activeId.set(blockId);
      sendMsg({ t: 'focus', blockIds: [blockId] });
    },
    onBlurBlock() {
      sendMsg({ t: 'focus', blockIds: [] });
    },
    onCreateParagraphAfter(afterId) {
      const nextP = paragraph('');
      dispatchOps([{ op: 'insert', at: { after: afterId }, blocks: [nextP] }]);
      activeId.set(nextP.id);
      draftText.set('');
    },
  };

  const runReplaceAll = () => {
    if (!doc || !findQuery.value) return;
    const needle = findQuery.value;
    const ops: AnyJinshuOp[] = [];
    for (const blk of doc.blocks) {
      if ((blk.type === 'paragraph' || blk.type === 'heading') && plainText(blk.runs).includes(needle)) {
        ops.push({ op: 'edit', id: blk.id, find: needle, replace: replaceQuery.value, all: true, noteWord: note('@jinshu.note.replace', { text: needle }) });
      } else if (blk.type === 'code' && blk.code.includes(needle)) {
        ops.push({ op: 'edit', id: blk.id, field: 'code', find: needle, replace: replaceQuery.value, all: true, noteWord: note('@jinshu.note.replace', { text: needle }) });
      }
    }
    if (ops.length > 0) dispatchOps(ops);
  };

  const renderCtx: BlockRenderContext = {
    mode: readOnly ? 'view' : 'edit',
    idScope: scope,
    testScope: props['data-testid'],
    resolveSrc: props.resolveSrc,
    filePolicy: props.filePolicy ?? liveState.filePolicy,
    mediaPolicy: props.mediaPolicy,
    customBlocks: props.customBlocks,
    customInlines: props.customInlines,
    isSelectable: true,
    targetBlockId: props.nav?.state.targetBlockId,
    onLinkPress: props.onLinkPress,
    onInPageLink: id => props.nav?.scrollTo(id),
    onFileAction: props.onFileAction,
    onOpenImage: readOnly ? undefined : blockId => selectBlock(selectedNonTextId.value === blockId ? null : blockId),
    onUpdateCustomAttrs: readOnly
      ? undefined
      : (blockId, attrs) => {
          dispatchOps([{ op: 'update', id: blockId, attrs, noteWord: note('@jinshu.note.custom') }]);
        },
  };

  const activeBlock = doc && activeId.value ? doc.blocks.find(b => b.id === activeId.value) ?? null : null;
  const canFormatInline = Boolean(activeBlock && (activeBlock.type === 'paragraph' || activeBlock.type === 'heading'));
  const outlineHeadings = doc ? extractOutline(doc) : [];

  const hasMedia = liveState.hasMedia === true;
  const uploads = liveState.uploads ?? {};
  const filteredSlash = SLASH_ITEMS.filter(item => {
    if (item.media && !hasMedia) return false;
    const q = slashQuery.value.toLowerCase();
    return q === '' || blockTypeLabel(item.id).toLowerCase().includes(q) || item.keywords.includes(q);
  });

  const matchingBlockIds = new Set<string>();
  if (showFind.value && findQuery.value && doc) {
    const q = findQuery.value.toLowerCase();
    for (const blk of doc.blocks) {
      const txt = blk.type === 'code' ? blk.code : 'runs' in blk && blk.runs ? plainText(blk.runs) : '';
      if (txt.toLowerCase().includes(q)) matchingBlockIds.add(blk.id);
    }
  }

  const saveLabel =
    props.saveStatus === 'saving'
      ? jinshuText('@jinshu.editor.save.saving')
      : props.saveStatus === 'failed'
        ? jinshuText('@jinshu.editor.save.failed')
        : isDirty
          ? jinshuText('@jinshu.editor.save.unsaved')
          : jinshuText('@jinshu.editor.save.saved');

  const hasGutter = !readOnly && isDesktop;

  // AI suggestions grouped under anchor block
  const sugByBlock = new Map<string, Suggestion[]>();
  const unanchoredSugs: Suggestion[] = [];
  if (doc) {
    for (const sug of suggestions) {
      const anchor = suggestionAnchor(sug, doc);
      if (anchor) sugByBlock.set(anchor, [...(sugByBlock.get(anchor) ?? []), sug]);
      else unanchoredSugs.push(sug);
    }
  }
  const anchoredOrder = doc ? doc.blocks.filter(b => sugByBlock.has(b.id)).map(b => b.id) : [];

  // What needs the person, in one queue: conflicts, an agent's save request, then suggestions in document order.
  type QueueItem =
    | { readonly kind: 'conflict'; readonly id: string }
    | { readonly kind: 'save'; readonly origin: Origin; readonly summary?: string }
    | { readonly kind: 'suggestion'; readonly suggestion: Suggestion; readonly anchor: string | null };
  const queue: QueueItem[] = [
    ...conflicts.map(c => ({ kind: 'conflict' as const, id: c.id })),
    ...(saveRequest ? [{ kind: 'save' as const, origin: saveRequest.origin, ...(saveRequest.summary ? { summary: saveRequest.summary } : {}) }] : []),
    ...anchoredOrder.flatMap(anchor => (sugByBlock.get(anchor) ?? []).map(sug => ({ kind: 'suggestion' as const, suggestion: sug, anchor }))),
    ...unanchoredSugs.map(sug => ({ kind: 'suggestion' as const, suggestion: sug, anchor: null })),
  ];
  const queueIndex = queue.length === 0 ? 0 : Math.min(queueCursor.value, queue.length - 1);
  const queueItem = queue[queueIndex];
  const stepQueue = (step: number) => {
    const next = (queueIndex + step + queue.length) % queue.length;
    queueCursor.set(next);
    const item = queue[next];
    if (item.kind === 'suggestion' && item.anchor) reveal(`jinshu-${scope}-s-${item.anchor}`);
  };
  const answerSave = (accepted: boolean) => {
    live.set({ ...liveState, saveRequest: null });
    sendMsg({ t: 'save', action: accepted ? 'confirmAgentSave' : 'cancelAgentSave' });
  };

  // Blocks the agent changed, by top-level block, for marks and the bar's View, which steps through them.
  const changedIds: string[] = [];
  if (doc) {
    for (const batch of agentBatches) {
      for (const id of batch.blockIds) {
        const top = topLevelOf(id, doc);
        if (top && !changedIds.includes(top)) changedIds.push(top);
      }
    }
  }
  const changedOrder = doc ? doc.blocks.filter(b => changedIds.includes(b.id)).map(b => b.id) : [];
  const latestBatch = agentBatches[agentBatches.length - 1];
  const dropBatch = (batchId: string) =>
    live.set({ ...liveState, agentBatches: agentBatches.filter(b => b.batchId !== batchId) });
  const revealBlock = (blockId: string) => reveal(`jinshu-${scope}-r-${blockId}`);
  const nextChange = () => {
    if (changedOrder.length === 0) return;
    const next = changeCursor.value % changedOrder.length;
    changeCursor.set(next + 1);
    revealBlock(changedOrder[next]);
  };

  // Where each agent is working; a stream's mark rides on its preview instead.
  const markByBlock = new Map<string, AgentActivity>();
  const topMarks: AgentActivity[] = [];
  if (doc) {
    for (const act of activities) {
      if (act.streamId) continue;
      const top = act.blockIds.length > 0 ? topLevelOf(act.blockIds[0], doc) : null;
      if (top) markByBlock.set(top, act);
      else topMarks.push(act);
    }
  }
  const renderActivity = (act: AgentActivity, isTop: boolean) => (
    <view key={act.id} role="status" className={cx(e.activity.base, isTop && e.activity.top)}>
      <view aria-hidden className={e.activity.dot} />
      <text className={e.activity.text}>
        {jinshuText(act.kind === 'reading' ? '@jinshu.editor.activity.reading' : '@jinshu.editor.activity.writing', { name: act.origin.label ?? act.origin.id })}
      </text>
      {act.kind === 'writing' && (
        <Button
          label={jinshuText('@jinshu.editor.activity.stop')}
          startIcon="stop"
          size="sm"
          variant="ghost"
          onClick={() => sendMsg(act.streamId ? { t: 'streamAbort', streamId: act.streamId } : { t: 'stopAgent', originId: act.origin.id })}
        />
      )}
    </view>
  );

  const renderSuggestionCard = (sug: Suggestion, block: JinshuBlock | undefined) => {
    const isStale = sug.kind === 'block' && sug.status === 'stale';
    let prefixCtx = '';
    let suffixCtx = '';
    let removed = '';
    let added = '';
    if (sug.kind === 'range') {
      const before = blockPlainText(block);
      prefixCtx = before.slice(Math.max(0, sug.from - 12), sug.from);
      if (sug.from > 12) prefixCtx = `…${prefixCtx}`;
      removed = before.slice(sug.from, sug.to);
      added = plainText(sug.insert);
      // What the two sides share is context, so only the change is struck and inserted.
      let head = 0;
      while (head < removed.length && head < added.length && removed[head] === added[head]) head++;
      let tail = 0;
      while (tail < removed.length - head && tail < added.length - head && removed[removed.length - 1 - tail] === added[added.length - 1 - tail]) tail++;
      const from = sug.from + head;
      const to = sug.to - tail;
      prefixCtx = before.slice(Math.max(0, from - 12), from);
      if (from > 12) prefixCtx = `…${prefixCtx}`;
      suffixCtx = before.slice(to, Math.min(before.length, to + 12));
      if (to + 12 < before.length) suffixCtx = `${suffixCtx}…`;
      removed = removed.slice(head, removed.length - tail);
      added = added.slice(head, added.length - tail);
    } else {
      for (const op of sug.ops) {
        if (op.op === 'edit') {
          removed += op.find;
          added += op.replace;
        } else if (op.op === 'replace' || op.op === 'insert') {
          added += (op.blocks ?? []).map(b => ('runs' in b && Array.isArray(b.runs) ? plainText(b.runs as never) : `[${b.type}]`)).join(' ');
          if (op.op === 'replace') removed += blockPlainText(block);
        } else if (op.op === 'delete') {
          removed += blockPlainText(block) || jinshuText('@jinshu.editor.suggestion.wholeBlock');
        }
      }
    }
    // Same text, other marks: show the new formatting itself rather than an empty diff.
    const isFormatOnly = sug.kind === 'range' && !removed && !added;
    const fallbackSummary = !removed && !added && !isFormatOnly;
    return (
      <view key={sug.id} className={e.suggestion.base}>
        <view className={e.suggestion.header}>
          <view className={e.suggestion.meta}>
            <Badge label={originName(sug.origin)} tone="info" size="sm" />
            {isStale && <Badge label={jinshuText('@jinshu.editor.suggestion.stale')} tone="warning" size="sm" />}
            <text className={e.suggestion.note}>{noteText(sug) ?? jinshuText('@jinshu.editor.suggestion.untitled')}</text>
          </view>
          <view className={e.suggestion.actions}>
            {!isStale && <Button label={jinshuText('@jinshu.editor.suggestion.accept')} size="sm" variant="tonal" onClick={() => sendMsg({ t: 'review', action: 'accept', suggestionId: sug.id })} />}
            <Button label={jinshuText('@jinshu.editor.suggestion.reject')} size="sm" variant="ghost" onClick={() => sendMsg({ t: 'review', action: 'reject', suggestionId: sug.id })} />
          </view>
        </view>
        {isFormatOnly && sug.kind === 'range' ? (
          <view className={e.suggestion.formatOnly}>
            <text className={e.suggestion.formatLabel}>{jinshuText('@jinshu.editor.suggestion.formatOnly')}</text>
            <text className={e.suggestion.diff}>{renderInlineRuns(sug.insert, { blockId: sug.block })}</text>
          </view>
        ) : fallbackSummary ? (
          <text className={e.suggestion.summary}>{suggestionSummary(sug)}</text>
        ) : (
          <text className={e.suggestion.diff}>
            {prefixCtx && <span className={e.suggestion.context}>{prefixCtx}</span>}
            {removed && <span className={e.suggestion.removed}>{removed}</span>}
            {removed && added ? ' ' : ''}
            {added && <span className={e.suggestion.added}>{added}</span>}
            {suffixCtx && <span className={e.suggestion.context}>{suffixCtx}</span>}
          </text>
        )}
      </view>
    );
  };

  const turnIntoValue = currentTurnInto(activeBlock);
  const canTurnInto = turnIntoValue !== null;
  const turnIntoLabel = blockTypeLabel(TURN_INTO.find(t => t.id === turnIntoValue)?.id ?? 'paragraph');
  const turnIntoItems: DropdownMenuEntry[] = TURN_INTO.map(t => ({
    id: `turn-${t.id}`,
    label: blockTypeLabel(t.id),
    ...(t.id === turnIntoValue ? { icon: 'check' as const } : {}),
    onClick: () => {
      if (!activeBlock) return;
      commitActiveDraft(activeBlock, draftText.value);
      const op = turnIntoOp(activeBlock, t.id);
      if (op) dispatchOps([op]);
      if (t.id === 'paragraph' || t.id.startsWith('h')) draftText.set(shorthandOf(activeBlock));
      else activeId.set(null);
    },
  }));

  const markButton = (glyph: string, label: string, mark: string, textClass: string, close = mark) => (
    <MarkGlyphButton
      glyph={glyph}
      label={label}
      disabled={!canFormatInline}
      active={canFormatInline && hasTrailingShorthandMark(draftText.value, mark)}
      textClass={textClass}
      onClick={() => applyFormatButton(mark, close)}
    />
  );

  const insertPanel = (key: string, afterId: string | null) => (
    <InsertBlockPanel
      nameKey={key}
      hasMedia={hasMedia}
      onPickBlock={kind => {
        insertBlockAt(afterId, makeBlockForSlash(kind, seed));
        gutterAddId.set(null);
        popEpoch.set(popEpoch.value + 1);
      }}
      onPickMedia={kind => {
        pickMedia(kind, afterId);
        gutterAddId.set(null);
        popEpoch.set(popEpoch.value + 1);
      }}
      onPickTable={(rows, cols) => {
        insertBlockAt(afterId, makeBlockForSlash('table', seed, rows, cols));
        gutterAddId.set(null);
        popEpoch.set(popEpoch.value + 1);
      }}
    />
  );

  const linkPopover = (
    <Popover
      key={`link-pop-${popEpoch.value}`}
      label={jinshuText('@jinshu.editor.link.insert')}
      title={jinshuText('@jinshu.editor.link.title')}
      content={
        <view className={e.link.base}>
          <input
            aria-label={jinshuText('@jinshu.editor.link.text')}
            value={linkLabel.value}
            placeholder={jinshuText('@jinshu.editor.link.textPlaceholder')}
            onInput={v => linkLabel.set(v)}
            className={e.link.text}
          />
          <input
            aria-label={jinshuText('@jinshu.editor.link.address')}
            value={linkHref.value}
            placeholder={jinshuText('@jinshu.editor.link.addressPlaceholder')}
            onInput={v => linkHref.set(v)}
            className={e.link.href}
          />
          {outlineHeadings.length > 0 && (
            <view className={e.link.sections}>
              <text className={e.link.sectionsTitle}>{jinshuText('@jinshu.editor.link.sections')}</text>
              <view className={e.link.sectionList}>
                {outlineHeadings.slice(0, 6).map(headingItem => (
                  <button
                    key={headingItem.id}
                    onClick={() => {
                      if (!linkLabel.value) linkLabel.set(headingItem.text);
                      linkHref.set(`#${headingItem.id}`);
                    }}
                    className={cx(e.link.section, linkHref.value === `#${headingItem.id}` && e.link.sectionSelected)}
                  >
                    <text className={e.link.sectionText}>{headingItem.text}</text>
                  </button>
                ))}
              </view>
            </view>
          )}
          <view className={e.link.actions}>
            <Button
              label={jinshuText('@jinshu.editor.insert')}
              size="sm"
              variant="primary"
              isDisabled={!linkHref.value.trim() || linkHref.value.trim() === 'https://'}
              onClick={() => {
                const text = linkLabel.value.trim() || linkHref.value.trim();
                insertInlineSnippet(`[${text}](${linkHref.value.trim()})`);
                linkLabel.set('');
                linkHref.set('https://');
                popEpoch.set(popEpoch.value + 1);
              }}
            />
          </view>
        </view>
      }
    >
      <IconButton label={jinshuText('@jinshu.editor.link.title')} icon="link" size="sm" isDisabled={!canFormatInline} />
    </Popover>
  );

  const mathPopover = (
    <Popover
      key={`math-pop-${popEpoch.value}`}
      label={jinshuText('@jinshu.editor.math.insert')}
      title={jinshuText('@jinshu.editor.blockType.math')}
      content={
        <view className={e.math.base}>
          <view className={e.math.preview}>
            <MathFormulaView latex={mathDraft.value || 'E = mc^2'} />
          </view>
          <textarea
            aria-label={jinshuText('@jinshu.editor.math.source')}
            value={mathDraft.value}
            rows={2}
            maxRows={5}
            placeholder={jinshuText('@jinshu.editor.math.sourcePlaceholder', { example: '\\frac{a}{b}' })}
            onInput={v => mathDraft.set(v)}
            className={e.math.source}
          />
          <view className={e.math.actions}>
            <Button
              label={jinshuText('@jinshu.editor.math.inline')}
              size="sm"
              variant="secondary"
              isDisabled={!canFormatInline}
              onClick={() => {
                if (mathDraft.value.trim()) insertInlineSnippet(`$${mathDraft.value.trim()}$`);
                popEpoch.set(popEpoch.value + 1);
              }}
            />
            <Button
              label={jinshuText('@jinshu.editor.math.block')}
              size="sm"
              variant="primary"
              onClick={() => {
                insertBlockAt(activeBlock?.id ?? null, mathBlock(mathDraft.value.trim() || 'E = mc^2'));
                popEpoch.set(popEpoch.value + 1);
              }}
            />
          </view>
        </view>
      }
    >
      <IconButton label={jinshuText('@jinshu.editor.blockType.math')} icon={editorIcons.math} size="sm" />
    </Popover>
  );

  /** Block type and inline marks for the block being edited: above it on desktop, above the keyboard on phones. */
  const formatControls = (
    <>
      <DropdownMenu label={jinshuText('@jinshu.editor.format.turnInto')} size="sm" items={turnIntoItems}>
        <Button label={turnIntoLabel} endIcon="chevron-down" size="sm" variant="ghost" isDisabled={!canTurnInto} data-testid={props['data-testid'] && `${props['data-testid']}-turn-into`} />
      </DropdownMenu>
      <ToolbarSep />
      {markButton('B', jinshuText('@jinshu.editor.format.bold'), '**', e.markGlyph.mark.bold)}
      {markButton('I', jinshuText('@jinshu.editor.format.italic'), '*', e.markGlyph.mark.italic)}
      {markButton('U', jinshuText('@jinshu.editor.format.underline'), '++', e.markGlyph.mark.underline)}
      {markButton('S', jinshuText('@jinshu.editor.format.strike'), '~~', e.markGlyph.mark.strike)}
      {markButton('</>', jinshuText('@jinshu.editor.format.code'), '`', e.markGlyph.mark.code)}
      <IconButton
        label={jinshuText('@jinshu.editor.format.highlight')}
        icon={editorIcons.highlight}
        size="sm"
        isDisabled={!canFormatInline}
        variant={canFormatInline && hasTrailingShorthandMark(draftText.value, '==') ? 'tonal' : 'ghost'}
        onClick={() => applyFormatButton('==')}
      />
      {linkPopover}
      <ToolbarSep />
      {markButton('x²', jinshuText('@jinshu.editor.format.superscript'), '^', e.markGlyph.mark.superscript)}
      {markButton('x₂', jinshuText('@jinshu.editor.format.subscript'), '~', e.markGlyph.mark.subscript)}
      {mathPopover}
    </>
  );

  const endEditing = () => {
    if (activeBlock) blurActive(activeBlock);
    else activeId.set(null);
  };
  const previewImage = doc && previewId.value ? doc.blocks.find((b): b is Extract<JinshuBlock, { type: 'image' }> => b.id === previewId.value && b.type === 'image') : undefined;
  const isEditingText = !readOnly && activeBlock !== null && isInlineTextBlock(activeBlock) && activeBlock.type !== 'math';
  const keyboardInset = env.keyboard.height;

  return (
    <view data-testid={props['data-testid']} className={cx(e.root.base, props.className, !isDesktop && isEditingText && e.root.keyboardPad)}>
      {/* Header: the document bar and everything an agent needs the person to see stay in view while scrolling. */}
      <view className={cx(e.header.base, stickyToolbar && e.header.sticky)}>
      {/* Document-level bar: history, insert, the app's actions, find, save. Block editing lives at the block. */}
      {!readOnly && (
        <view
          role="toolbar"
          aria-label={jinshuText('@jinshu.editor.toolbar')}
          className={e.toolbar.base}
        >
          <view className={e.toolbarInner.base}>
            <view className={e.toolbarStart.base}>
              {chan && (
                <>
                  <IconButton label={jinshuText('@jinshu.editor.undo')} icon="undo" size="sm" isDisabled={!liveState.canUndo} onClick={() => sendMsg({ t: 'history', action: 'undo' })} />
                  <IconButton label={jinshuText('@jinshu.editor.redo')} icon="redo" size="sm" isDisabled={!liveState.canRedo} onClick={() => sendMsg({ t: 'history', action: 'redo' })} />
                  <ToolbarSep />
                </>
              )}
              <Popover key={`add-pop-${popEpoch.value}`} label={jinshuText('@jinshu.editor.insert')} content={insertPanel(`top-${docKey}`, activeBlock?.id ?? null)}>
                <Button label={jinshuText('@jinshu.editor.insert')} startIcon="add" size="sm" variant="ghost" data-testid={props['data-testid'] && `${props['data-testid']}-toolbar-add`} />
              </Popover>
            </view>

            <view className={e.toolbarEnd.base}>
              {props.toolbarActions}
              <IconButton label={jinshuText('@jinshu.editor.find.toggle')} icon="search" size="sm" variant={showFind.value ? 'tonal' : 'ghost'} onClick={() => showFind.set(!showFind.value)} />
              {chan && (
                <>
                  <Popover
                    label={jinshuText('@jinshu.editor.history')}
                    title={jinshuText('@jinshu.editor.history.titleCount', { count: history.length })}
                    alignment="end"
                    content={
                      <view className={e.history.base}>
                        {history.length === 0 ? (
                          <text className={e.history.empty}>{jinshuText('@jinshu.editor.history.empty')}</text>
                        ) : (
                          <scroll className={e.history.scroll}>
                            <view className={e.history.list}>
                              {[...history].reverse().map(entry => (
                                <view key={entry.batchId} className={e.history.entry}>
                                  <view className={e.history.entryBody}>
                                    <text className={cx(e.history.note, entry.undone ? e.history.noteUndone : e.history.noteDone)}>
                                      {noteText(entry) || jinshuText('@jinshu.editor.history.entry', { count: entry.blockIds.length })}
                                    </text>
                                    <text className={e.history.origin}>{originName(entry.origin)}</text>
                                  </view>
                                  {!entry.undone && (
                                    <Button label={jinshuText('@jinshu.editor.undo')} size="sm" variant="ghost" onClick={() => sendMsg({ t: 'history', action: 'undoBatch', batchId: entry.batchId })} />
                                  )}
                                </view>
                              ))}
                            </view>
                          </scroll>
                        )}
                      </view>
                    }
                  >
                    <IconButton label={jinshuText('@jinshu.editor.history')} icon="clock" size="sm" />
                  </Popover>
                  {isDirty || props.saveStatus === 'failed' || props.saveStatus === 'saving' ? (
                    <Button
                      label={props.saveStatus === 'saving' ? jinshuText('@jinshu.editor.save.saving') : props.saveStatus === 'failed' ? jinshuText('@jinshu.editor.save.retry') : jinshuText('@jinshu.editor.save')}
                      size="sm"
                      variant={props.saveStatus === 'failed' ? 'destructive' : 'primary'}
                      isDisabled={props.saveStatus === 'saving'}
                      onClick={() => sendMsg({ t: 'save', action: 'save' })}
                    />
                  ) : (
                    <view className={e.saved.base}>
                      <vector aria-hidden paths={SAVED_PATH} viewBox={[0, 0, 16, 16]} className={e.saved.glyph} />
                      <text className={e.saved.label}>{saveLabel}</text>
                    </view>
                  )}
                </>
              )}
            </view>
          </view>
        </view>
      )}

      {/* Find & Replace bar */}
      {!readOnly && showFind.value && (
        <view className={e.findBar.base}>
          <input
            aria-label={jinshuText('@jinshu.editor.find.field')}
            value={findQuery.value}
            placeholder={jinshuText('@jinshu.editor.find.placeholder')}
            autoFocus
            onInput={v => findQuery.set(v)}
            className={e.findBar.field}
          />
          <input
            aria-label={jinshuText('@jinshu.editor.find.replaceWith')}
            value={replaceQuery.value}
            placeholder={jinshuText('@jinshu.editor.find.replaceWith')}
            onInput={v => replaceQuery.set(v)}
            className={e.findBar.field}
          />
          {findQuery.value !== '' && <text className={e.findBar.count}>{jinshuText('@jinshu.editor.find.count', { count: matchingBlockIds.size })}</text>}
          <Button label={jinshuText('@jinshu.editor.find.replaceAll')} size="sm" variant="tonal" isDisabled={!findQuery.value || matchingBlockIds.size === 0} onClick={runReplaceAll} />
          <IconButton label={jinshuText('@jinshu.editor.find.close')} icon="close" size="sm" onClick={() => showFind.set(false)} />
        </view>
      )}

      {/* An agent's direct change: who, how much, view and undo for that batch only */}
      {latestBatch && (
        <view role="status" className={e.agentBar.base}>
          <text className={e.agentBar.text}>
            {jinshuText(latestBatch.away ? '@jinshu.editor.agent.changedAway' : '@jinshu.editor.agent.changed', {
              name: latestBatch.origin.label ?? latestBatch.origin.id,
              count: latestBatch.blockIds.length,
            })}
          </text>
          <view className={e.agentBar.actions}>
            {latestBatch.blockIds.length > 0 && doc && (
              <Button
                label={jinshuText('@jinshu.editor.agent.view')}
                size="sm"
                variant="ghost"
                onClick={nextChange}
              />
            )}
            <Button
              label={jinshuText('@jinshu.editor.agent.undo')}
              startIcon="undo"
              size="sm"
              variant="tonal"
              onClick={() => {
                sendMsg({ t: 'history', action: 'undoBatch', batchId: latestBatch.batchId });
                dropBatch(latestBatch.batchId);
              }}
            />
            <IconButton label={jinshuText('@jinshu.editor.agent.dismiss')} icon="close" size="sm" onClick={() => dropBatch(latestBatch.batchId)} />
          </view>
        </view>
      )}

      {topMarks.map(act => renderActivity(act, true))}

      {/* Everything waiting for the person, one item at a time; nothing an agent asks for opens a modal */}
      {queueItem && (
        <view role="status" className={e.queue.base}>
          <view className={e.queue.head}>
            <view className={e.queue.nav}>
              <text className={e.queue.count}>{jinshuText('@jinshu.editor.queue.count', { count: queue.length })}</text>
              {queue.length > 1 && (
                <>
                  <IconButton label={jinshuText('@jinshu.editor.queue.previous')} icon="chevron-up" size="sm" onClick={() => stepQueue(-1)} />
                  <IconButton label={jinshuText('@jinshu.editor.queue.next')} icon="chevron-down" size="sm" onClick={() => stepQueue(1)} />
                </>
              )}
            </view>
            {suggestions.length > 1 && (
              <view className={e.queue.actions}>
                <Button label={jinshuText('@jinshu.editor.review.rejectAll')} size="sm" variant="ghost" onClick={() => sendMsg({ t: 'review', action: 'rejectAll' })} />
                <Button label={jinshuText('@jinshu.editor.review.acceptAll')} size="sm" variant="tonal" onClick={() => sendMsg({ t: 'review', action: 'acceptAll' })} />
              </view>
            )}
          </view>
          {queueItem.kind === 'conflict' ? (
            <view className={e.queue.item}>
              <view className={e.queue.body}>
                <text className={e.queue.title}>{jinshuText('@jinshu.editor.queue.conflict')}</text>
              </view>
              <view className={e.queue.actions}>
                <Button label={jinshuText('@jinshu.editor.conflict.theirs')} size="sm" variant="ghost" onClick={() => sendMsg({ t: 'save', action: 'resolveConflict', resolution: 'theirs' })} />
                <Button label={jinshuText('@jinshu.editor.conflict.copy')} size="sm" variant="ghost" onClick={() => sendMsg({ t: 'save', action: 'resolveConflict', resolution: 'copy' })} />
                <Button label={jinshuText('@jinshu.editor.conflict.mine')} size="sm" variant="tonal" onClick={() => sendMsg({ t: 'save', action: 'resolveConflict', resolution: 'mine' })} />
              </view>
            </view>
          ) : queueItem.kind === 'save' ? (
            <view className={e.queue.item}>
              <view className={e.queue.body}>
                <text className={e.queue.title}>{jinshuText('@jinshu.editor.queue.save', { name: queueItem.origin.label ?? queueItem.origin.id })}</text>
                {queueItem.summary && <text className={e.queue.detail}>{queueItem.summary}</text>}
              </view>
              <view className={e.queue.actions}>
                <Button label={jinshuText('@jinshu.editor.queue.save.decline')} size="sm" variant="ghost" onClick={() => answerSave(false)} />
                <Button label={jinshuText('@jinshu.editor.queue.save.confirm')} size="sm" variant="tonal" onClick={() => answerSave(true)} />
              </view>
            </view>
          ) : (
            <view className={e.queue.item}>
              <view className={e.queue.body}>
                <text className={e.queue.title}>
                  {jinshuText('@jinshu.editor.queue.suggestion', {
                    name: queueItem.suggestion.origin.label ?? queueItem.suggestion.origin.id,
                    note: noteText(queueItem.suggestion) ?? jinshuText('@jinshu.editor.suggestion.untitled'),
                  })}
                </text>
                {queueItem.suggestion.downgraded && (
                  <text className={e.queue.detail}>
                    {jinshuText(queueItem.suggestion.downgraded === 'focus' ? '@jinshu.editor.queue.downgraded.focus' : '@jinshu.editor.queue.downgraded.large')}
                  </text>
                )}
              </view>
              <view className={e.queue.actions}>
                {queueItem.anchor && (
                  <Button label={jinshuText('@jinshu.editor.review.show')} size="sm" variant="ghost" onClick={() => reveal(`jinshu-${scope}-s-${queueItem.anchor}`)} />
                )}
                <Button label={jinshuText('@jinshu.editor.suggestion.reject')} size="sm" variant="ghost" onClick={() => sendMsg({ t: 'review', action: 'reject', suggestionId: queueItem.suggestion.id })} />
                {!(queueItem.suggestion.kind === 'block' && queueItem.suggestion.status === 'stale') && (
                  <Button label={jinshuText('@jinshu.editor.suggestion.accept')} size="sm" variant="tonal" onClick={() => sendMsg({ t: 'review', action: 'accept', suggestionId: queueItem.suggestion.id })} />
                )}
              </view>
            </view>
          )}
        </view>
      )}
      </view>
      {unanchoredSugs.length > 0 && <view className={e.unanchored.base}>{unanchoredSugs.map(sug => renderSuggestionCard(sug, undefined))}</view>}

      {/* Phone: the format bar rides above the keyboard while a text block is edited. */}
      {!isDesktop && isEditingText && (
        <view
          role="toolbar"
          aria-label={jinshuText('@jinshu.editor.format.toolbar')}
          className={e.keyboardBar.base}
          style={keyboardInset > 0 ? { bottom: keyboardInset } : { bottom: -env.safeArea.bottom, paddingBottom: env.safeArea.bottom + 4 }}
        >
          <scroll className={e.keyboardBar.scroll}>
            <view className={e.keyboardBar.controls}>
              {formatControls}
              <ToolbarSep />
              <Popover key={`kb-add-${popEpoch.value}`} label={jinshuText('@jinshu.editor.insert')} content={insertPanel(`kb-${docKey}`, activeBlock?.id ?? null)}>
                <IconButton label={jinshuText('@jinshu.editor.insert.block')} icon="add" size="sm" />
              </Popover>
            </view>
          </scroll>
          <Button label={jinshuText('@jinshu.editor.done')} size="sm" variant="tonal" onClick={endEditing} data-testid={props['data-testid'] && `${props['data-testid']}-done`} />
        </view>
      )}

      {/* Streaming preview */}
      {streamPreview && streamPreview.status === 'streaming' && (
        <view className={e.stream.base}>
          {renderActivity({ id: streamPreview.streamId, origin: streamPreview.origin, kind: 'writing', blockIds: [], streamId: streamPreview.streamId }, false)}
          {streamPreview.blocks.map((blk, idx) =>
            renderBlock(blk, idx > 0 ? streamPreview.blocks[idx - 1].type : null, renderCtx),
          )}
        </view>
      )}

      {/* Calm, Centered Publication Canvas */}
      {doc === null ? (
        <text className={e.loading.base}>{jinshuText('@jinshu.editor.loading')}</text>
      ) : (
        <view className={e.canvas.base}>
          {/* Directly editable Document Title sharing the exact same gutter track as body blocks */}
          <view className={e.row.base}>
            {!readOnly && isDesktop && <view className={e.gutter.spacer} />}
            <view className={e.row.body}>
            {!readOnly && editingTitle.value ? (
              <input
                aria-label={jinshuText('@jinshu.editor.title')}
                value={titleDraft.value}
                autoFocus
                placeholder={jinshuText('@jinshu.editor.title.untitled')}
                onInput={v => titleDraft.set(v)}
                onBlur={() => {
                  editingTitle.set(false);
                  if (titleDraft.value !== (doc.title ?? '')) {
                    dispatchOps([{ op: 'setMeta', title: titleDraft.value, noteWord: note('@jinshu.note.title') }]);
                  }
                }}
                onKeyDown={{
                  Enter: () => {
                    editingTitle.set(false);
                    if (titleDraft.value !== (doc.title ?? '')) {
                      dispatchOps([{ op: 'setMeta', title: titleDraft.value, noteWord: note('@jinshu.note.title') }]);
                    }
                    if (doc.blocks[0] && isInlineTextBlock(doc.blocks[0])) focusBlock(doc.blocks[0]);
                  },
                  Escape: () => editingTitle.set(false),
                }}
                className={cx(e.title.input, r.docTitle.base)}
              />
            ) : !readOnly ? (
              <button
                aria-label={jinshuText('@jinshu.editor.title.edit')}
                onClick={() => {
                  titleDraft.set(doc.title ?? '');
                  editingTitle.set(true);
                }}
                className={e.title.button}
              >
                <text role="heading" headingLevel={1} className={cx(r.docTitle.base, !doc.title && e.title.empty)}>
                  {doc.title || jinshuText('@jinshu.editor.title.untitled')}
                </text>
              </button>
            ) : (
              doc.title && (
                <text role="heading" headingLevel={1} className={r.docTitle.base}>
                  {doc.title}
                </text>
              )
            )}
            </view>
          </view>

          {doc.blocks.map((block, idx) => {
            const isActive = !readOnly && activeId.value === block.id;
            const isHovered = !readOnly && hoveredId.value === block.id;
            const isSlashOpen = !readOnly && slashAfterId.value === block.id;
            const isFindMatch = matchingBlockIds.has(block.id);
            const isNonTextSelected = !readOnly && selectedNonTextId.value === block.id;
            const prevType = idx > 0 ? doc.blocks[idx - 1].type : null;
            const spacing = blockTopSpacing(prevType, block.type);
            const showGutter = !readOnly && isDesktop && (isHovered || isActive || gutterAddId.value === block.id);

            const blockMenuEntries: DropdownMenuEntry[] = [
              ...(currentTurnInto(block) !== null
                ? ([
                    {
                      type: 'group',
                      label: jinshuText('@jinshu.editor.blockMenu.turnInto'),
                      items: TURN_INTO.filter(t => t.id !== currentTurnInto(block)).map(t => ({
                        id: `t-${t.id}`,
                        label: blockTypeLabel(t.id),
                        onClick: () => {
                          const op = turnIntoOp(block, t.id);
                          if (op) dispatchOps([op]);
                        },
                      })),
                    },
                    { type: 'divider' },
                  ] as DropdownMenuEntry[])
                : []),
              ...(idx > 0
                ? [{ id: 'mv-up', label: jinshuText('@jinshu.editor.blockMenu.moveUp'), icon: 'arrow-up' as const, onClick: () => dispatchOps([{ op: 'move', id: block.id, to: { before: doc.blocks[idx - 1].id }, noteWord: note('@jinshu.note.moveUp') }]) }]
                : []),
              ...(idx < doc.blocks.length - 1
                ? [{ id: 'mv-dn', label: jinshuText('@jinshu.editor.blockMenu.moveDown'), icon: 'arrow-down' as const, onClick: () => dispatchOps([{ op: 'move', id: block.id, to: { after: doc.blocks[idx + 1].id }, noteWord: note('@jinshu.note.moveDown') }]) }]
                : []),
              {
                id: 'dup',
                label: jinshuText('@jinshu.editor.blockMenu.duplicate'),
                icon: 'copy',
                onClick: () => dispatchOps([{ op: 'insert', at: { after: block.id }, blocks: [duplicateBlockInput(block)], noteWord: note('@jinshu.note.duplicate') }]),
              },
              ...(isObjectBlock(block) && !isNonTextSelected
                ? [{ id: 'sel', label: jinshuText('@jinshu.editor.blockMenu.properties'), icon: 'edit' as const, onClick: () => { selectBlock(block.id); inspecting.set(isMediaBlock(block)); } }]
                : []),
              ...(block.type === 'details'
                ? [{
                    id: 'det-open',
                    label: jinshuText('@jinshu.editor.blockMenu.detailsStart', { open: block.defaultOpen ? 'yes' : 'no' }),
                    icon: 'chevron-down' as const,
                    onClick: () => dispatchOps([{ op: 'update', id: block.id, attrs: { defaultOpen: block.defaultOpen ? null : true }, noteWord: note('@jinshu.note.detailsStart') }]),
                  }]
                : []),
              ...(props.onAskAgent
                ? [{ id: 'ask', label: jinshuText('@jinshu.editor.blockMenu.askAgent'), icon: 'message' as const, onClick: () => props.onAskAgent?.([block.id]) }]
                : []),
              { type: 'divider' },
              {
                id: 'del',
                label: jinshuText('@jinshu.editor.blockMenu.delete'),
                icon: 'delete',
                variant: 'destructive',
                onClick: () => {
                  dispatchOps([{ op: 'delete', id: block.id, noteWord: note('@jinshu.note.delete') }]);
                  if (activeId.value === block.id) activeId.set(null);
                },
              },
            ];

            return (
              <view
                key={block.id}
                id={`jinshu-${scope}-r-${block.id}`}
                onLayout={
                  block.id === props.openAt && openedAt.value !== block.id
                    ? () => {
                        if (openedAt.value === block.id) return;
                        openedAt.set(block.id);
                        revealBlock(block.id);
                      }
                    : undefined
                }
                onPointerEnter={isDesktop ? () => hoveredId.set(block.id) : undefined}
                onPointerLeave={isDesktop ? () => { if (hoveredId.value === block.id) hoveredId.set(null); } : undefined}
                className={cx(spacing, e.block.base, isFindMatch && e.block.findMatch)}
              >
                {markByBlock.has(block.id) && <view className={cx(hasGutter && e.gutter.indent)}>{renderActivity(markByBlock.get(block.id)!, false)}</view>}
                <view className={e.row.base}>
                  {/* Quiet left gutter: true floating Popover (+) and DropdownMenu (⋮) with zero layout shift */}
                  {!readOnly && isDesktop && (
                    <view className={e.gutter.base}>
                      {showGutter && (
                        <>
                          <Popover
                            key={`gpop-${block.id}-${popEpoch.value}`}
                            label={jinshuText('@jinshu.editor.insert.below')}
                            onOpenChange={open => gutterAddId.set(open ? block.id : null)}
                            content={insertPanel(`g-${block.id}`, block.id)}
                          >
                            <IconButton label={jinshuText('@jinshu.editor.insert.below')} icon="add" size="sm" />
                          </Popover>

                          <DropdownMenu label={jinshuText('@jinshu.editor.blockMenu')} size="sm" items={blockMenuEntries}>
                            <IconButton label={jinshuText('@jinshu.editor.blockMenu')} icon="more-vertical" size="sm" />
                          </DropdownMenu>
                        </>
                      )}
                    </view>
                  )}

                  {/* Block body: 1:1 geometry between read & edit states */}
                  <view className={e.block.body}>
                    {changedIds.includes(block.id) && <view aria-hidden className={e.block.agentChanged} />}
                    {!readOnly && block.type === 'columns' ? (
                      <ColumnsBlockEditor
                        block={block}
                        cb={blockCb}
                        renderFallback={child => <view>{renderBlock(child, null, renderCtx)}</view>}
                      />
                    ) : !readOnly && block.type === 'list' ? (
                      <ListBlockEditor block={block} cb={blockCb} />
                    ) : !readOnly && block.type === 'callout' ? (
                      <CalloutBlockEditor block={block} cb={blockCb} />
                    ) : !readOnly && block.type === 'quote' ? (
                      <QuoteBlockEditor block={block} cb={blockCb} />
                    ) : !readOnly && block.type === 'details' ? (
                      <DetailsBlockEditor block={block} cb={blockCb} />
                    ) : !readOnly && block.type === 'code' ? (
                      <CodeBlockEditor block={block} cb={blockCb} />
                    ) : isActive && block.type === 'math' ? (
                      /* quiet-md-inspired Live Equation Editor: rendered formula stays live above LaTeX source */
                      <view className={e.mathEditor.base}>
                        <scroll className={e.mathEditor.scroll}>
                          <view className={e.mathEditor.preview}>
                            <MathFormulaView latex={draftText.value || block.latex} />
                          </view>
                        </scroll>
                        <view className={e.mathEditor.source}>
                          <textarea
                            aria-label={jinshuText('@jinshu.editor.edit.source', { type: 'math' })}
                            value={draftText.value}
                            autoFocus
                            rows={1}
                            maxRows={6}
                            placeholder={jinshuText('@jinshu.editor.math.editPlaceholder', { example: '\\int_0^\\infty e^{-x^2}\\,dx = \\frac{\\sqrt{\\pi}}{2}' })}
                            onInput={val => draftText.set(val)}
                            onBlur={() => commitOnBlur(block)}
                            onKeyDown={{
                              Escape: () => blurActive(block),
                            }}
                            className={e.mathEditor.field}
                          />
                          <view className={e.mathEditor.footer}>
                            <text className={e.mathEditor.hint}>{jinshuText('@jinshu.editor.math.hint')}</text>
                            <Button label={jinshuText('@jinshu.editor.done')} size="sm" variant="tonal" onClick={() => blurActive(block)} />
                          </view>
                        </view>
                      </view>
                    ) : isActive && isInlineTextBlock(block) ? (
                      <view className={e.textEditor.base}>
                        {isDesktop && (
                          <view
                            role="toolbar"
                            aria-label={jinshuText('@jinshu.editor.format.toolbar')}
                            className={e.formatBar.base}
                          >
                            {formatControls}
                          </view>
                        )}
                      <textarea
                        aria-label={jinshuText('@jinshu.editor.edit.source', { type: block.type })}
                        value={draftText.value}
                        autoFocus
                        rows={1}
                        maxRows={24}
                        placeholder={block.type === 'paragraph' ? jinshuText('@jinshu.editor.text.placeholder') : jinshuText('@jinshu.editor.heading.placeholder')}
                        onInput={val => handleLiveInput(block, val)}
                        onBlur={() => commitOnBlur(block)}
                        onKeyDown={{
                          Enter: () => {
                            if (isSlashOpen && filteredSlash.length > 0) {
                              const picked = filteredSlash[Math.min(slashIndex.value, filteredSlash.length - 1)];
                              applySlashChoice(block.id, picked.id, true);
                              return;
                            }
                            commitActiveDraft(block, draftText.value);
                            const nextP = paragraph('');
                            dispatchOps([{ op: 'insert', at: { after: block.id }, blocks: [nextP] }]);
                            activeId.set(nextP.id);
                            draftText.set('');
                          },
                          ArrowUp: () => {
                            if (isSlashOpen && filteredSlash.length > 0) {
                              slashIndex.set((slashIndex.value - 1 + filteredSlash.length) % filteredSlash.length);
                              return;
                            }
                            if (idx > 0 && isInlineTextBlock(doc.blocks[idx - 1])) {
                              focusBlock(doc.blocks[idx - 1]);
                            }
                          },
                          ArrowDown: () => {
                            if (isSlashOpen && filteredSlash.length > 0) {
                              slashIndex.set((slashIndex.value + 1) % filteredSlash.length);
                              return;
                            }
                            if (idx < doc.blocks.length - 1 && isInlineTextBlock(doc.blocks[idx + 1])) {
                              focusBlock(doc.blocks[idx + 1]);
                            }
                          },
                          Escape: () => {
                            if (isSlashOpen) {
                              slashAfterId.set(null);
                              slashQuery.set('');
                            } else {
                              blurActive(block);
                            }
                          },
                        }}
                        className={
                          block.type === 'heading'
                            ? cx(e.textEditor.field, r.heading.base, r.heading.level[block.level])
                            : cx(e.textEditor.field, r.paragraph.base)
                        }
                      />
                      </view>
                    ) : !readOnly && isInlineTextBlock(block) ? (
                      <view className={e.textBlock.base}>
                        {renderBlock(block, null, renderCtx)}
                        <button
                          aria-label={jinshuText('@jinshu.editor.edit.block', { id: block.id })}
                          onClick={() => focusBlock(block)}
                          className={e.textBlock.hit}
                        />
                      </view>
                    ) : !readOnly ? (
                      <>
                        <view className={cx(e.objectBlock.base, isNonTextSelected && e.objectBlock.selected)}>
                          {isCoarse && !isNonTextSelected ? (
                            <>
                              {renderBlock(block, null, { ...renderCtx, mode: 'view', onOpenImage: undefined })}
                              {/* Laid over the block, not around it: a table's scroll cannot sit inside a button. */}
                              <button
                                aria-label={jinshuText('@jinshu.editor.selection.select')}
                                onClick={() => {
                                  endEditing();
                                  selectBlock(block.id);
                                }}
                                className={e.objectBlock.hit}
                              />
                            </>
                          ) : block.type === 'table' ? (
                            <TableBlockEditor block={block} cb={blockCb} />
                          ) : (
                            renderBlock(block, null, renderCtx)
                          )}
                          {uploads[block.id] !== undefined && (
                            <ProgressBar label={jinshuText('@jinshu.editor.uploading')} value={Math.round(uploads[block.id] * 100)} valueLabel={`${Math.round(uploads[block.id] * 100)}%`} size="sm" />
                          )}
                        </view>
                        {isNonTextSelected && (
                          <view role="toolbar" aria-label={jinshuText('@jinshu.editor.selection.toolbar')} className={e.selectionBar.base} data-testid={props['data-testid'] && `${props['data-testid']}-selection-bar`}>
                            {idx > 0 && (
                              <IconButton label={jinshuText('@jinshu.editor.object.moveUp')} icon="arrow-up" size="sm" onClick={() => dispatchOps([{ op: 'move', id: block.id, to: { before: doc.blocks[idx - 1].id }, noteWord: note('@jinshu.note.moveUp') }])} />
                            )}
                            {idx < doc.blocks.length - 1 && (
                              <IconButton label={jinshuText('@jinshu.editor.object.moveDown')} icon="arrow-down" size="sm" onClick={() => dispatchOps([{ op: 'move', id: block.id, to: { after: doc.blocks[idx + 1].id }, noteWord: note('@jinshu.note.moveDown') }])} />
                            )}
                            {isMediaBlock(block) && hasMedia && (
                              <IconButton label={jinshuText('@jinshu.editor.selection.replace')} icon="refresh" size="sm" onClick={() => pickMedia(block.type as MediaKind, null, block.id)} />
                            )}
                            {isMediaBlock(block) && (
                              <IconButton
                                label={jinshuText('@jinshu.editor.blockMenu.properties')}
                                icon="edit"
                                size="sm"
                                variant={inspecting.value ? 'tonal' : 'ghost'}
                                onClick={() => inspecting.set(!inspecting.value)}
                              />
                            )}
                            {block.type === 'image' && (
                              <IconButton label={jinshuText('@jinshu.editor.selection.preview')} icon="expand" size="sm" onClick={() => previewId.set(block.id)} />
                            )}
                            <IconButton
                              label={jinshuText('@jinshu.editor.object.delete')}
                              icon="delete"
                              size="sm"
                              onClick={() => {
                                dispatchOps([{ op: 'delete', id: block.id, noteWord: note('@jinshu.note.delete') }]);
                                selectBlock(null);
                              }}
                            />
                            <view aria-hidden className={e.selectionBar.sep} />
                            <DropdownMenu label={jinshuText('@jinshu.editor.blockMenu')} size="sm" items={blockMenuEntries.filter(entry => !('id' in entry) || !['mv-up', 'mv-dn', 'del', 'sel'].includes(entry.id ?? ''))}>
                              <IconButton label={jinshuText('@jinshu.editor.blockMenu')} icon="more" size="sm" />
                            </DropdownMenu>
                            <IconButton label={jinshuText('@jinshu.editor.selection.clear')} icon="close" size="sm" onClick={() => selectBlock(null)} />
                          </view>
                        )}
                        {isNonTextSelected && inspecting.value && block.type === 'image' && <ImageBlockInspector block={block} cb={blockCb} />}
                        {isNonTextSelected && inspecting.value && (block.type === 'video' || block.type === 'audio' || block.type === 'file') && (
                          <MediaBlockInspector block={block} cb={blockCb} />
                        )}
                      </>
                    ) : (
                      renderBlock(block, null, renderCtx)
                    )}
                  </view>
                </view>

                {/* In-place AI suggestions for this block */}
                {sugByBlock.has(block.id) && (
                  <view id={`jinshu-${scope}-s-${block.id}`} className={cx(hasGutter && e.gutter.indent, e.suggestions.base)}>
                    {(sugByBlock.get(block.id) ?? []).map(sug => renderSuggestionCard(sug, block))}
                  </view>
                )}

                {/* Keyboard-navigable `/` Slash Command Palette */}
                {isSlashOpen && (
                  <view className={cx(hasGutter && e.gutter.indent, e.slashMenu.base)}>
                    <text className={e.slashMenu.hint}>
                      {slashQuery.value ? jinshuText('@jinshu.editor.slash.matching', { query: slashQuery.value }) : jinshuText('@jinshu.editor.slash.hint')}
                    </text>
                    <view className={e.slashMenu.grid}>
                      {filteredSlash.map((item, sIdx) => {
                        const isSelectedSlash = sIdx === Math.min(slashIndex.value, Math.max(0, filteredSlash.length - 1));
                        return (
                          <button
                            key={item.id}
                            onClick={() => applySlashChoice(block.id, item.id, isActive && draftText.value.startsWith('/'))}
                            className={cx(e.slashMenu.item, isSelectedSlash ? e.slashMenu.itemSelected : e.slashMenu.itemIdle)}
                          >
                            <text className={e.slashMenu.badge}>{item.badge}</text>
                            <text className={e.slashMenu.label}>{blockTypeLabel(item.id)}</text>
                          </button>
                        );
                      })}
                    </view>
                  </view>
                )}
              </view>
            );
          })}

          {/* Clickable trailing canvas space */}
          {!readOnly && (
            <button
              aria-label={jinshuText('@jinshu.editor.trailing')}
              onClick={() => {
                selectBlock(null);
                const last = doc.blocks[doc.blocks.length - 1];
                if (last && last.type === 'paragraph' && plainText(last.runs) === '') {
                  focusBlock(last);
                  return;
                }
                const nextP = paragraph('');
                dispatchOps([{ op: 'insert', at: { end: 'doc' }, blocks: [nextP] }]);
                activeId.set(nextP.id);
                draftText.set('');
              }}
              className={cx(hasGutter && e.gutter.indent, e.trailing.base)}
            >
              <text className={e.trailing.hint}>{jinshuText('@jinshu.editor.trailing.hint')}</text>
            </button>
          )}
        </view>
      )}


      {previewImage && (
        <Lightbox
          isOpen
          onOpenChange={open => {
            if (!open) previewId.set(null);
          }}
          index={0}
          onIndexChange={() => previewId.set(previewId.value)}
          label={jinshuText('@jinshu.editor.selection.preview')}
          images={[{ src: props.resolveSrc ? props.resolveSrc(previewImage.src) : previewImage.src, alt: previewImage.alt ?? '' }]}
        />
      )}

      {/* Leave guard confirmation dialog */}
      <Dialog
        isOpen={guard.pending !== null}
        onOpenChange={open => {
          if (!open) guard.answer(false);
        }}
        label={jinshuText('@jinshu.editor.leave.title')}
        description={jinshuText('@jinshu.editor.leave.description')}
        actions={
          <>
            <Button label={jinshuText('@jinshu.editor.leave.stay')} variant="secondary" onClick={() => guard.answer(false)} />
            <Button
              label={jinshuText('@jinshu.editor.leave.save')}
              variant="primary"
              onClick={() => {
                sendMsg({ t: 'save', action: 'save' });
                guard.answer(true);
              }}
            />
            <Button label={jinshuText('@jinshu.editor.leave.discard')} variant="destructive" onClick={() => guard.answer(true)} />
          </>
        }
      />
    </view>
  );
}
