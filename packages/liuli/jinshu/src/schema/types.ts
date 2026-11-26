/**
 * Core Jinshu Document Schema types (@liulidesign/jinshu/schema).
 * Zero UI dependencies; shared across Logic, View, CLI, and Node tests.
 */

/** How a document is stored: Jinshu JSON, or a Markdown file holding the subset Markdown can say. */
export type JinshuFormat = 'json' | 'markdown';

/** Current document format version; documents with a higher `version` are read-only. */
export const JINSHU_DOC_VERSION = 1;

export interface JinshuDocument {
  /** Format version; never rewritten by normalize. A newer one makes the document read-only. */
  readonly version: number;
  readonly id?: string;
  readonly title?: string;
  /** BCP 47 language tag; omit to follow the app language. */
  readonly lang?: string;
  /** App-defined metadata passed through without interpretation. */
  readonly meta?: Readonly<Record<string, unknown>>;
  readonly blocks: readonly JinshuBlock[];
}

export type HighlightTone = 'accent' | 'warning' | 'success' | 'danger';
export type ScriptKind = 'sub' | 'super';

/** Flat inline text run with boolean/value mark attributes (no nested marks array). */
export interface InlineRun {
  readonly text: string;
  readonly bold?: true;
  readonly italic?: true;
  readonly underline?: true;
  readonly strike?: true;
  readonly code?: true;
  readonly script?: ScriptKind;
  readonly highlight?: HighlightTone;
  /** External URL or in-page `#block-id`. */
  readonly link?: string;
}

export type InlineMarkKey = keyof Omit<InlineRun, 'text'>;

/** Soft line break (Shift+Enter). `\n` is forbidden inside `InlineRun.text`. */
export interface HardBreak {
  readonly break: true;
}

/** Inline extension atom registered via `defineInline`. */
export interface InlineAtom {
  /** Unique across the document (blocks, items, rows and atoms share one id space). */
  readonly id: string;
  readonly inline: `x-${string}-${string}`;
  readonly version: number;
  readonly attrs: Readonly<Record<string, unknown>>;
  readonly fallback: string;
}

/** Built-in inline LaTeX math atom. */
export interface InlineMath {
  readonly math: string;
}

export type Inline = InlineRun | HardBreak | InlineAtom | InlineMath;

/** Inline kinds, discriminated by which one of `text` / `break` / `math` / `inline` is present. */
export type InlineKind = 'text' | 'break' | 'math' | 'atom';

export interface BlockBase {
  readonly id: string;
}

export interface ListItem {
  readonly id: string;
  readonly checked?: boolean;
  readonly runs: readonly Inline[];
  readonly blocks?: readonly JinshuBlock[];
}

export interface TableRow {
  readonly id: string;
  readonly cells: readonly (readonly Inline[])[];
}

export interface ColumnItem {
  readonly id: string;
  readonly blocks: readonly JinshuBlock[];
}

export interface MediaChapter {
  readonly atMs: number;
  readonly title: string;
}

export interface VideoSubtitle {
  readonly lang: string;
  readonly label: string;
  readonly src: string;
}

export type CalloutTone = 'info' | 'tip' | 'warning' | 'danger';
export type ListStyle = 'bullet' | 'ordered' | 'task';
export type TableAlign = 'start' | 'center' | 'end';
export type ImageSize = 'small' | 'medium' | 'full';
export type FileDisplay = 'attachment' | 'inline';
export type HeadingLevel = 1 | 2 | 3 | 4;

export interface ParagraphBlock extends BlockBase {
  readonly type: 'paragraph';
  readonly runs: readonly Inline[];
}

export interface HeadingBlock extends BlockBase {
  readonly type: 'heading';
  readonly level: HeadingLevel;
  readonly runs: readonly Inline[];
}

export interface QuoteBlock extends BlockBase {
  readonly type: 'quote';
  readonly blocks: readonly JinshuBlock[];
}

export interface CalloutBlock extends BlockBase {
  readonly type: 'callout';
  readonly tone: CalloutTone;
  readonly title?: readonly Inline[];
  readonly blocks: readonly JinshuBlock[];
}

export interface DetailsBlock extends BlockBase {
  readonly type: 'details';
  readonly summary: readonly Inline[];
  readonly defaultOpen?: boolean;
  readonly blocks: readonly JinshuBlock[];
}

export interface ListBlock extends BlockBase {
  readonly type: 'list';
  readonly style: ListStyle;
  readonly start?: number;
  readonly items: readonly ListItem[];
}

export interface CodeBlock extends BlockBase {
  readonly type: 'code';
  readonly language: string;
  readonly code: string;
  readonly filename?: string;
  readonly highlightLines?: readonly number[];
}

export interface MathBlock extends BlockBase {
  readonly type: 'math';
  readonly latex: string;
}

export interface TableBlock extends BlockBase {
  readonly type: 'table';
  readonly align?: readonly TableAlign[];
  readonly widths?: readonly number[];
  /** `rows[0]` is the header row; omitted when there is none. */
  readonly headerRow?: true;
  readonly rows: readonly TableRow[];
}

export interface ColumnsBlock extends BlockBase {
  readonly type: 'columns';
  readonly widths?: readonly number[];
  readonly columns: readonly ColumnItem[];
}

export interface DividerBlock extends BlockBase {
  readonly type: 'divider';
}

export interface ImageBlock extends BlockBase {
  readonly type: 'image';
  readonly src: string;
  readonly alt?: string;
  readonly width?: number;
  readonly height?: number;
  readonly caption?: readonly Inline[];
  readonly size?: ImageSize;
}

export interface VideoBlock extends BlockBase {
  readonly type: 'video';
  readonly src: string;
  readonly poster?: string;
  readonly durationMs?: number;
  readonly caption?: readonly Inline[];
  readonly chapters?: readonly MediaChapter[];
  readonly subtitles?: readonly VideoSubtitle[];
}

export interface AudioBlock extends BlockBase {
  readonly type: 'audio';
  readonly src: string;
  readonly title?: string;
  readonly cover?: string;
  readonly durationMs?: number;
  readonly chapters?: readonly MediaChapter[];
  readonly transcript?: readonly JinshuBlock[];
}

export interface FileBlock extends BlockBase {
  readonly type: 'file';
  readonly display?: FileDisplay;
  readonly src: string;
  readonly name: string;
  readonly mimeType?: string;
  readonly sizeBytes?: number;
  readonly pageCount?: number;
  readonly description?: string;
}

export interface ExtensionBlock extends BlockBase {
  readonly type: `x-${string}-${string}`;
  readonly version: number;
  readonly attrs: Readonly<Record<string, unknown>>;
  readonly fallback: string;
  readonly runs?: readonly Inline[];
  readonly blocks?: readonly JinshuBlock[];
}

export type JinshuBlock =
  | ParagraphBlock
  | HeadingBlock
  | QuoteBlock
  | CalloutBlock
  | DetailsBlock
  | ListBlock
  | CodeBlock
  | MathBlock
  | TableBlock
  | ColumnsBlock
  | DividerBlock
  | ImageBlock
  | VideoBlock
  | AudioBlock
  | FileBlock
  | ExtensionBlock;

export type BuiltinBlockType = Exclude<JinshuBlock['type'], `x-${string}-${string}`>;

/** Minimal JSON Schema subset used by `defineBlock` / `defineInline` for validation and AI tool export. */
export interface JsonSchema<T = unknown> {
  readonly type?: 'object' | 'string' | 'number' | 'integer' | 'boolean' | 'array' | 'null';
  readonly description?: string;
  readonly properties?: Readonly<Record<string, JsonSchema<unknown>>>;
  readonly required?: readonly string[];
  readonly items?: JsonSchema<unknown>;
  readonly enum?: readonly unknown[];
  readonly minimum?: number;
  readonly maximum?: number;
  readonly minLength?: number;
  readonly maxLength?: number;
  readonly additionalProperties?: boolean | JsonSchema<unknown>;
  readonly _type?: T;
}

export interface BlockSpec<TAttrs = Record<string, unknown>> {
  readonly type: `x-${string}-${string}`;
  readonly version: number;
  readonly attrsSchema: JsonSchema<TAttrs>;
  readonly migrate?: (attrs: unknown, from: number) => TAttrs;
  readonly content: 'none' | 'inline' | { readonly blocks: string };
  readonly aiDescription: string;
  readonly aiExample?: Readonly<Record<string, unknown>>;
  readonly fallbackText: (attrs: TAttrs) => string;
}

export interface InlineSpec<TAttrs = Record<string, unknown>> {
  readonly inline: `x-${string}-${string}`;
  readonly version: number;
  readonly attrsSchema: JsonSchema<TAttrs>;
  readonly migrate?: (attrs: unknown, from: number) => TAttrs;
  readonly aiDescription: string;
  readonly aiExample?: Readonly<Record<string, unknown>>;
  readonly fallbackText: (attrs: TAttrs) => string;
}

export interface JinshuRegistry {
  readonly blocks: ReadonlyMap<string, BlockSpec<any>>;
  readonly inlines: ReadonlyMap<string, InlineSpec<any>>;
}

export type BlockAttrsOf<TSpec> = TSpec extends BlockSpec<infer A> ? A : Record<string, unknown>;

export interface BlockProps<TSpec extends BlockSpec<any> = BlockSpec<any>> {
  readonly block: Omit<ExtensionBlock, 'attrs'> & { readonly attrs: BlockAttrsOf<TSpec> };
  readonly emit: (type: string, data: unknown) => void;
  readonly mode: 'view' | 'edit';
  readonly onUpdateAttrs?: (attrs: Partial<BlockAttrsOf<TSpec>>) => void;
}

/** Distributive Omit preserving union variants. */
export type DistributiveOmit<T, K extends keyof any> = T extends any ? Omit<T, K> : never;

export type BlockInput = DistributiveOmit<JinshuBlock, 'id' | 'fallback' | 'version' | 'blocks' | 'items' | 'columns' | 'rows'> & {
  readonly id?: string;
  readonly fallback?: string;
  readonly version?: number;
  readonly rows?: readonly { readonly id?: string; readonly cells: readonly (readonly Inline[])[] }[];
  readonly blocks?: readonly BlockInput[];
  readonly items?: readonly {
    readonly id?: string;
    readonly checked?: boolean;
    readonly runs: readonly Inline[];
    readonly blocks?: readonly BlockInput[];
  }[];
  readonly columns?: readonly {
    readonly id?: string;
    readonly blocks: readonly BlockInput[];
  }[];
};

export type Position =
  | { readonly before: string }
  | { readonly after: string }
  | { readonly start: string | 'doc' }
  | { readonly end: string | 'doc' };

export type EditField =
  | 'runs'
  | 'code'
  | 'title'
  | 'summary'
  | 'caption'
  | { readonly cell: readonly [rowId: string, col: number] };

/** A history note in no one language: a Jinshu word key and its arguments, worded when drawn. */
export interface NoteWord {
  readonly key: `@jinshu.note.${string}`;
  readonly args?: Readonly<Record<string, string | number>>;
}

/** `note` is free text from whoever wrote the op; `noteWord` is the editor's own, worded in the reader's language. */
export type OpCommon = { readonly note?: string; readonly noteWord?: NoteWord };

export type MarkupOrBlocks =
  | { readonly markup: string; readonly blocks?: never }
  | { readonly blocks: readonly BlockInput[]; readonly markup?: never };

export type JinshuOp = OpCommon & (
  | ({ readonly op: 'insert'; readonly at: Position } & MarkupOrBlocks)
  | ({ readonly op: 'replace'; readonly id: string } & MarkupOrBlocks)
  | {
      readonly op: 'edit';
      readonly id: string;
      readonly field?: EditField;
      readonly lineRange?: readonly [start: number, end: number];
      readonly find: string;
      readonly replace: string;
      readonly all?: boolean;
    }
  | { readonly op: 'update'; readonly id: string; readonly attrs: Readonly<Record<string, unknown>> }
  | {
      /** Rows are addressed by id. insert: `rowId` names the new row, `before`/`after` anchor it (default: append). */
      readonly op: 'tableRow';
      readonly id: string;
      readonly action: 'insert' | 'delete' | 'move';
      readonly rowId?: string;
      readonly before?: string;
      readonly after?: string;
      readonly cells?: readonly (readonly Inline[])[];
    }
  | {
      readonly op: 'tableCol';
      readonly id: string;
      readonly action: 'insert' | 'delete' | 'move';
      readonly index: number;
      readonly toIndex?: number;
      readonly header?: readonly Inline[];
      readonly align?: TableAlign;
      readonly width?: number;
    }
  | { readonly op: 'delete'; readonly id: string }
  | { readonly op: 'move'; readonly id: string; readonly to: Position }
  /** `meta` is merged key by key; a key set to `null` is removed. */
  | { readonly op: 'setMeta'; readonly title?: string; readonly lang?: string; readonly meta?: Readonly<Record<string, unknown>> }
  | { readonly op: 'rewrite'; readonly markup: string }
);

export interface TextTarget {
  readonly blockId: string;
  readonly field?: EditField;
}

export type JinshuTextOp = OpCommon & (
  | { readonly op: 'text'; readonly target: TextTarget; readonly from: number; readonly to: number; readonly insert: readonly Inline[] }
  | {
      readonly op: 'mark';
      readonly target: TextTarget;
      readonly from: number;
      readonly to: number;
      readonly add?: Partial<Omit<InlineRun, 'text'>>;
      readonly remove?: readonly InlineMarkKey[];
    }
  | { readonly op: 'split'; readonly id: string; readonly at: number; readonly newId: string }
  | { readonly op: 'merge'; readonly id: string }
  | {
      readonly op: 'setType';
      readonly id: string;
      readonly type: 'paragraph' | 'heading' | 'quote' | 'list' | 'callout' | 'code';
      readonly level?: HeadingLevel;
      readonly style?: ListStyle;
      readonly tone?: CalloutTone;
      readonly language?: string;
      readonly childId?: string;
    }
);

export type AnyJinshuOp = JinshuOp | JinshuTextOp;

export type ProblemCode =
  | 'NOT_FOUND'
  | 'STALE_BLOCK'
  | 'NO_MATCH'
  | 'AMBIGUOUS_MATCH'
  | 'PARSE'
  | 'INVALID'
  | 'REJECTED'
  | 'LIMIT'
  /** An op names a field the target's type does not have, or a structural field `update` cannot set. */
  | 'UNKNOWN_FIELD'
  /** The document or an extension block is newer than this code; it renders but cannot be edited. */
  | 'VERSION'
  /** The person stopped this agent's edits; it reads the document again before editing. */
  | 'STOPPED'
  /** The document's file format cannot hold this, or it could not be read or written exactly. */
  | 'FORMAT';

/** A file-format rule a `FORMAT` problem is about, for words a person reads; `message` stays for agents. */
export type FormatRule =
  // Writing: what a Markdown file cannot say.
  | 'titleLine'
  | 'frontMatterLine'
  | 'cellBreak'
  | 'cellCodePipe'
  | 'inlineMath'
  | 'inlineInexact'
  | 'codeLanguage'
  | 'imageLine'
  | 'listStart'
  | 'checkedItem'
  | 'tableHeader'
  // Reading: what a Markdown file said that the document keeps differently.
  | 'inlineImage'
  | 'linkAsText'
  | 'linkTitle'
  | 'emptyAlt'
  | 'blockInList'
  | 'headingInQuote'
  | 'headingLevel'
  | 'codeInfo'
  | 'rawHtml'
  | 'alertType'
  | 'calloutTitle'
  | 'listDepth'
  | 'tableCells'
  | 'unreadable';

/** What a `FORMAT` problem is about: a block type, a mark, a field (of a block, or of the document), or a rule. */
export type FormatSubject =
  | { readonly block: string }
  | { readonly mark: InlineMarkKey | 'math' | 'atom' }
  | { readonly field: string; readonly block?: string }
  | { readonly rule: FormatRule };

export interface Problem {
  readonly code: ProblemCode;
  readonly severity: 'error' | 'warning';
  readonly opIndex?: number;
  readonly id?: string;
  readonly message: string;
  readonly current?: string;
  /** For `FORMAT`: what it is about, so a page can word it for a person. */
  readonly subject?: FormatSubject;
}

export interface CheckOptions {
  readonly allowedSrcSchemes?: readonly string[];
  readonly allowedLxApps?: readonly string[];
  /** The document's file format; `markdown` refuses what a Markdown file cannot hold (problem `FORMAT`). */
  readonly format?: JinshuFormat;
}

export interface ApplyOpsOptions extends CheckOptions {
  readonly dryRun?: boolean;
}

export interface ApplyOpsResult {
  readonly doc: JinshuDocument;
  readonly problems: readonly Problem[];
}

export interface Origin {
  readonly kind: 'human' | 'agent' | 'external';
  readonly id: string;
  /** The name the person knows (Codex, Kimi); without it a human origin is the person themselves. */
  readonly label?: string;
  /** The agent conversation that made the change, as the host names it. */
  readonly conversation?: string;
  /** The app the agent reached Jinshu through. */
  readonly via?: string;
}

export interface OpBatch {
  readonly batchId: string;
  readonly origin: Origin;
  readonly base: number;
  readonly ops: readonly AnyJinshuOp[];
  readonly mode?: 'direct' | 'suggest';
}

export interface BlockSuggestion {
  readonly kind: 'block';
  readonly id: string;
  readonly origin: Origin;
  readonly base: number;
  readonly ops: readonly JinshuOp[];
  readonly status: 'pending' | 'stale';
  readonly note?: string;
  readonly noteWord?: NoteWord;
  /** Why a direct agent edit became this suggestion: it touched the block being edited, or it was too large. */
  readonly downgraded?: 'focus' | 'large';
}

export interface RangeSuggestion {
  readonly kind: 'range';
  readonly id: string;
  readonly origin: Origin;
  readonly block: string;
  readonly field?: EditField;
  readonly from: number;
  readonly to: number;
  readonly insert: readonly Inline[];
  readonly note?: string;
  readonly noteWord?: NoteWord;
  readonly downgraded?: 'focus' | 'large';
}

export type Suggestion = BlockSuggestion | RangeSuggestion;

export interface HistoryEntry {
  readonly batchId: string;
  readonly revision: number;
  readonly origin: Origin;
  readonly acceptedFrom?: Origin;
  readonly timestampMs: number;
  readonly blockIds: readonly string[];
  readonly note?: string;
  readonly noteWord?: NoteWord;
  readonly undone?: boolean;
}

export interface ConflictItem {
  readonly id: string;
  readonly blockId?: string;
  readonly kind: 'block' | 'version';
  readonly localBlock?: JinshuBlock;
  readonly remoteBlock?: JinshuBlock;
  readonly remoteDoc?: JinshuDocument;
  readonly remoteVersion?: string;
  readonly message: string;
}

export interface FilePolicy {
  readonly default?: FileDisplay;
  readonly inline?: 'allow' | 'never';
}

export interface MediaPolicy {
  readonly seek?: 'any' | 'played-only' | 'none';
  readonly maxRate?: number;
}

export type Observation = { readonly seq: number; readonly sessionId: string } & (
  | { readonly kind: 'dwell'; readonly blockId: string; readonly contentHash?: string; readonly dwellMs: number; readonly seenRatio: number; readonly idleMs: number }
  | { readonly kind: 'played'; readonly blockId: string; readonly src: string; readonly range: readonly [startMs: number, endMs: number]; readonly durationMs: number; readonly rate: number; readonly ended?: true }
  | { readonly kind: 'position'; readonly blockId: string; readonly offset: number }
  | { readonly kind: 'block'; readonly blockId: string; readonly type: string; readonly data: unknown }
);

export interface JinshuProgress {
  readonly seenSeq?: Readonly<Record<string, number>>;
  readonly position?: { readonly blockId: string; readonly offset: number };
  readonly blocks: Readonly<Record<string, { readonly dwellMs: number; readonly seenRatio: number; readonly contentHash?: string }>>;
  readonly media: Readonly<Record<string, { readonly src: string; readonly played: readonly (readonly [number, number])[]; readonly positionMs: number; readonly durationMs: number }>>;
}

export interface OutlineItem {
  readonly id: string;
  readonly level: HeadingLevel;
  readonly text: string;
}

export interface TemplateSlot {
  readonly block: string;
  readonly kind: 'inline' | 'blocks' | 'attrs';
  readonly description: string;
  readonly required?: boolean;
  readonly maxBlocks?: number;
}

export interface JinshuTemplate {
  readonly id: string;
  readonly title: string;
  readonly doc: JinshuDocument;
  readonly slots: Readonly<Record<string, TemplateSlot>>;
}
