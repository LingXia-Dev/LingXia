import type {
  AnyJinshuOp,
  BlockSuggestion,
  CheckOptions,
  ConflictItem,
  FilePolicy,
  HistoryEntry,
  JinshuBlock,
  JinshuDocument,
  JinshuOp,
  JinshuRegistry,
  JinshuTemplate,
  OpBatch,
  Origin,
  Position,
  Problem,
  Suggestion,
} from '../schema/types';
import type { AbortSignalLike } from './signal';

/** A file as `lx.pick.media` and `lx.pick.file` return it, so an adapter passes it on unchanged. */
export interface PickedFile {
  readonly path: string;
  readonly name: string;
  /** Bytes. */
  readonly size: number;
  /** MIME type. */
  readonly type: string;
}

export type PickResult =
  | {
      readonly src: string;
      readonly name?: string;
      readonly poster?: string;
      readonly durationMs?: number;
      readonly width?: number;
      readonly height?: number;
      readonly mimeType?: string;
      readonly sizeBytes?: number;
    }
  | { readonly file: PickedFile };

/**
 * How the editor gets media. The app owns the choice: a project's asset library, the system
 * picker (`lx.pick.media`), a recorder. Without an adapter the editor offers no media entries.
 */
export interface JinshuMediaAdapter {
  /** Runs when the author asks to insert or replace media; `null` means they cancelled. */
  readonly pick: (request: {
    readonly kind: 'image' | 'video' | 'audio' | 'file';
    readonly replacing?: JinshuBlock;
  }) => Promise<PickResult | null>;
  /** Uploads a picked local file; the block shows the local file meanwhile. Without it, the local path is kept. */
  readonly upload?: (
    file: PickedFile,
    onProgress: (ratio: number) => void,
  ) => Promise<{
    readonly src: string;
    readonly poster?: string;
    readonly durationMs?: number;
    readonly width?: number;
    readonly height?: number;
    readonly mimeType?: string;
    readonly sizeBytes?: number;
  }>;
}

export type BeforeApplyResult =
  | { readonly accept: true; readonly ops?: readonly AnyJinshuOp[] }
  | { readonly reject: string };

export interface BeforeApplyContext {
  readonly origin: Origin;
  readonly block?: JinshuBlock;
  readonly doc: JinshuDocument;
}

export type BeforeApplyHook = (op: AnyJinshuOp, ctx: BeforeApplyContext) => BeforeApplyResult;

export type ValidateBatchHook = (
  nextDoc: JinshuDocument,
  prevDoc: JinshuDocument,
  ctx: { readonly origin: Origin; readonly batch: OpBatch },
) => readonly Problem[];

export interface LoadResult {
  readonly doc: JinshuDocument;
  readonly version: string;
}

export type SaveResult =
  | { readonly ok: true; readonly version: string }
  | { readonly ok: false; readonly conflict: true; readonly remoteDoc: JinshuDocument; readonly remoteVersion: string }
  | { readonly ok: false; readonly conflict?: false; readonly error: string };

export interface JinshuDraftStorage {
  get(key: string): Promise<unknown>;
  set(key: string, value: unknown): Promise<void>;
  remove(key: string): Promise<void>;
}

export interface JinshuSourceOptions extends CheckOptions {
  readonly key: string;
  readonly registry?: JinshuRegistry;
  readonly initialDoc?: JinshuDocument;
  readonly initialVersion?: string;
  readonly load?: () => Promise<LoadResult>;
  readonly save?: (doc: JinshuDocument, baseVersion: string) => Promise<SaveResult>;
  /** Optional local key-value storage (such as `lx.storage`) for automatic crash-recovery draft persistence. */
  readonly storage?: JinshuDraftStorage;
  readonly filePolicy?: FilePolicy;
  readonly saveMode?: 'manual' | 'auto';
  readonly agentSave?: 'confirm' | 'allow' | 'deny';
  readonly template?: JinshuTemplate;
  readonly media?: JinshuMediaAdapter;
  readonly beforeApply?: BeforeApplyHook;
  readonly validateBatch?: ValidateBatchHook;
  /** Prompt-injection defense: force `rewrite` or >30% deletions by agents to `suggest`. Default `true`. */
  readonly guardLargeAgentEdits?: boolean;
}

export interface JinshuSourceState {
  readonly key: string;
  readonly doc: JinshuDocument;
  readonly revision: number;
  readonly savedRevision: number;
  readonly baseVersion: string;
  readonly dirty: boolean;
  readonly saving: boolean;
  readonly saveStatus: 'saved' | 'dirty' | 'saving' | 'failed';
  readonly saveError?: string;
  readonly problems: readonly Problem[];
  readonly suggestions: readonly Suggestion[];
  readonly conflicts: readonly ConflictItem[];
  readonly history: readonly HistoryEntry[];
  readonly canUndo: boolean;
  readonly canRedo: boolean;
  readonly focusIds: readonly string[];
  readonly pendingAgentSave?: { readonly origin: Origin; readonly summary?: string };
  /** Pages showing this document now. */
  readonly viewers: number;
  readonly activities: readonly AgentActivity[];
}

/** A person's decision on something an agent asked for, recorded at the revision it was made. */
export type ReviewRecord =
  | { readonly revision: number; readonly kind: 'suggestion'; readonly suggestionId: string; readonly accepted: boolean }
  | { readonly revision: number; readonly kind: 'save'; readonly origin: Origin; readonly accepted: boolean }
  | { readonly revision: number; readonly kind: 'stop'; readonly origin: Origin }
  /** The person took back (or restored) one of an agent's direct changes; `origin` is that agent. */
  | { readonly revision: number; readonly kind: 'undone' | 'redone'; readonly origin: Origin; readonly batchId: string; readonly blockIds: readonly string[] };

/** What an agent is doing in the document now, for the page's marker. */
export interface AgentActivity {
  readonly id: string;
  readonly origin: Origin;
  readonly kind: 'reading' | 'writing';
  /** Where; empty means the whole document. */
  readonly blockIds: readonly string[];
  /** Set while a `stream()` writes; Stop aborts it. Without it, Stop stops the origin's next edits. */
  readonly streamId?: string;
}

/** An agent's direct batch the page shows with its "undo this change" bar. */
export interface AgentBatch {
  readonly batchId: string;
  readonly revision: number;
  readonly origin: Origin;
  readonly blockIds: readonly string[];
  /** Landed while no page showed the document. */
  readonly away?: true;
}

export interface ForAgentOptions {
  readonly since?: number;
  readonly range?: { readonly from?: string; readonly to?: string };
  readonly search?: string;
  readonly outline?: boolean;
  /** The reader; a stop the person gave it is reported once and cleared. */
  readonly origin?: Origin;
}

export interface ForAgentResult {
  readonly revision: number;
  readonly title: string;
  readonly view: string;
  readonly focusIds: readonly string[];
  readonly reviews: readonly ReviewRecord[];
  /** Suggestion ids still waiting for the person. */
  readonly pending: readonly string[];
  /** An agent's save request still waiting for the person. */
  readonly saveWaiting: boolean;
  /** The person stopped this reader's edits since it last read. */
  readonly stopped?: true;
}

export interface JinshuStream {
  readonly streamId: string;
  push(chunk: string): { readonly blocks: readonly JinshuBlock[]; readonly problems: readonly Problem[] };
  finish(): { readonly revision: number; readonly batchId?: string; readonly problems: readonly Problem[] };
  abort(): { readonly suggestion?: BlockSuggestion };
}

// ============================================================================
// GPU Bridge Channel Protocol
// ============================================================================

export type BlockChange =
  | { readonly kind: 'insert'; readonly at: Position; readonly blocks: readonly JinshuBlock[] }
  | { readonly kind: 'replace'; readonly id: string; readonly block: JinshuBlock }
  | { readonly kind: 'delete'; readonly id: string }
  | { readonly kind: 'move'; readonly id: string; readonly to: Position }
  | { readonly kind: 'setMeta'; readonly title?: string; readonly lang?: string; readonly meta?: Readonly<Record<string, unknown>> };

export type ChannelClientMsg =
  | { readonly t: 'open'; readonly doc: string; readonly have?: number }
  | { readonly t: 'ops'; readonly batch: OpBatch }
  | { readonly t: 'focus'; readonly blockIds: readonly string[] }
  | { readonly t: 'review'; readonly action: 'accept' | 'reject' | 'acceptAll' | 'rejectAll'; readonly suggestionId?: string }
  | { readonly t: 'history'; readonly action: 'undo' | 'redo' | 'undoBatch'; readonly batchId?: string }
  | {
      readonly t: 'save';
      readonly action: 'save' | 'confirmAgentSave' | 'cancelAgentSave' | 'resolveConflict';
      readonly resolution?: 'mine' | 'theirs' | 'copy';
    }
  | { readonly t: 'streamAbort'; readonly streamId: string }
  /** The person stops an agent writing without a stream: its next edits are refused until it reads again. */
  | { readonly t: 'stopAgent'; readonly originId: string }
  | {
      readonly t: 'media';
      readonly action: 'pick' | 'upload';
      readonly kind?: 'image' | 'video' | 'audio' | 'file';
      readonly at?: Position;
      readonly replacingId?: string;
      readonly tempPath?: string;
    };

export type ChannelServerMsg =
  | { readonly t: 'snapshot'; readonly revision: number; readonly doc: JinshuDocument; readonly policy?: { readonly filePolicy?: FilePolicy; readonly hasMedia?: boolean } }
  | { readonly t: 'changes'; readonly revision: number; readonly changes: readonly BlockChange[] }
  | { readonly t: 'suggestions'; readonly suggestions: readonly Suggestion[] }
  | { readonly t: 'stream'; readonly streamId: string; readonly origin: Origin; readonly at: Position; readonly blocks: readonly JinshuBlock[]; readonly status: 'streaming' | 'done' | 'aborted' }
  | { readonly t: 'activity'; readonly activities: readonly AgentActivity[] }
  | { readonly t: 'agentBatches'; readonly batches: readonly AgentBatch[] }
  | { readonly t: 'conflicts'; readonly conflicts: readonly ConflictItem[] }
  | { readonly t: 'history'; readonly entries: readonly HistoryEntry[]; readonly canUndo: boolean; readonly canRedo: boolean }
  | { readonly t: 'saveRequest'; readonly origin: Origin; readonly summary?: string }
  | { readonly t: 'uploadProgress'; readonly blockId: string; readonly ratio: number }
  | { readonly t: 'ack'; readonly revision: number; readonly batchId: string }
  | { readonly t: 'reject'; readonly batchId: string; readonly problems: readonly Problem[]; readonly snapshot?: JinshuDocument; readonly revision?: number };

export interface MinimalChannelEndpoint {
  send(payload: ChannelServerMsg): void;
  close?(code?: string, reason?: string): void;
  on(event: 'data', handler: (payload: ChannelClientMsg) => void): () => void;
  on(event: 'close', handler: (info: { code: string; reason: string }) => void): () => void;
}

export interface JinshuSource {
  readonly key: string;
  readonly registry: JinshuRegistry;
  readonly filePolicy?: FilePolicy;
  readonly agentSave: 'confirm' | 'allow' | 'deny';
  getState(): JinshuSourceState;
  load(): Promise<JinshuSourceState>;
  save(): Promise<SaveResult>;
  /** Restores an unsaved crash-recovery draft from `options.storage` if one exists. */
  restoreDraft(): Promise<boolean>;
  apply(batch: OpBatch): { readonly revision: number; readonly batchId: string; readonly problems: readonly Problem[]; readonly suggestionId?: string };
  undo(batchId?: string, origin?: Origin, options?: { readonly force?: boolean }): { readonly revision: number; readonly problems: readonly Problem[] };
  redo(origin?: Origin): { readonly revision: number; readonly problems: readonly Problem[] };
  acceptSuggestion(suggestionId: string, humanOrigin?: Origin): { readonly revision: number; readonly problems: readonly Problem[] };
  rejectSuggestion(suggestionId: string): { readonly revision: number; readonly problems: readonly Problem[] };
  acceptAllSuggestions(humanOrigin?: Origin): { readonly revision: number; readonly problems: readonly Problem[] };
  rejectAllSuggestions(): { readonly revision: number; readonly problems: readonly Problem[] };
  setFocus(blockIds: readonly string[]): void;
  requestSave(origin: Origin, summary?: string): Promise<{ readonly saved: boolean; readonly pendingConfirmation?: boolean; readonly problems: readonly Problem[] }>;
  confirmAgentSave(): Promise<SaveResult>;
  cancelAgentSave(): void;
  resolveConflict(resolution: 'mine' | 'theirs' | 'copy'): Promise<SaveResult | null>;
  stream(options: { readonly at: Position; readonly origin: Origin; readonly mode?: 'direct' | 'suggest' }): JinshuStream;
  forAgent(options?: ForAgentOptions): ForAgentResult;
  /** Resolves once the revision passes `since` (an edit or a review), or when `signal` aborts. */
  waitForChange(since: number, signal: AbortSignalLike): Promise<void>;
  /** Shows what an agent is doing; a mark without a stream clears itself a few seconds after it is set. */
  setActivity(activity: AgentActivity | null, originId?: string): void;
  /** Stops an agent: aborts its stream, or refuses its edits until it reads again (`forAgent({ origin })`). */
  stopAgent(originId: string): void;
  serve(channel: MinimalChannelEndpoint): () => void;
  subscribe(listener: (state: JinshuSourceState) => void): () => void;
  close(): void;
}
