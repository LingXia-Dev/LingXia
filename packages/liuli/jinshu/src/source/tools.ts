import { exportAiSchema, newBlockId, walkBlocks } from '../schema/index';
import type { JinshuDocument } from '../schema/types';
import type { FilePolicy, JinshuOp, JinshuRegistry, Origin, Problem } from '../schema/types';
import { logicText } from '../i18n/logicText';
import type { AgentAssessment, AgentCall, AgentResult, AgentTool } from './agent';
import { anySignal, timeoutSignal } from './signal';
import type { JinshuSource } from './types';

export interface JinshuDocumentEntry {
  readonly key: string;
  readonly title: string;
}

/** The app's documents as agents reach them; the tools never depend on a page being open. */
export interface JinshuDocuments {
  /** Documents matching `query` (empty: the app's choice of recent ones). */
  list(query: string): readonly JinshuDocumentEntry[] | Promise<readonly JinshuDocumentEntry[]>;
  /** The shared source for `key`, the one a page showing it uses; `null` when there is none. */
  open(key: string): JinshuSource | null | Promise<JinshuSource | null>;
  /**
   * Where a person opens the document; results carry it so the agent can offer it. `at` is the
   * top-level block the change landed in: open the page there (`openAt` on the View or Editor).
   */
  pageOf?(key: string, at?: string): string | undefined;
  /** The source for `key` if already open, so listing never loads a document. */
  peek?(key: string): JinshuSource | undefined;
}

export interface JinshuToolsOptions {
  readonly documents: JinshuDocuments;
  /** Custom blocks the markup guide teaches. */
  readonly registry?: JinshuRegistry;
  readonly filePolicy?: FilePolicy;
}

type Args = Record<string, unknown>;

const LIST_LIMIT = 20;
const MAX_WAIT_MS = 25_000;

/** The agent behind a call: one origin per conversation, so undo and stop reach only its own edits. */
function originOf(call: AgentCall): Origin {
  return {
    kind: 'agent',
    id: `${call.caller.appId}/${call.conversation}`,
    label: call.agent,
    conversation: call.conversation,
    via: call.caller.name,
  };
}

function error(code: string, message: string): AgentResult {
  return { status: 'error', code, message };
}

function problemsResult(problems: readonly Problem[]): AgentResult {
  const errors = problems.filter(p => p.severity === 'error');
  const message = errors
    .map(p => `${p.code}${p.id ? ` (${p.id})` : ''}${p.opIndex !== undefined ? ` op ${p.opIndex}` : ''}: ${p.message}${p.current ? `\nCurrent block: ${p.current}` : ''}`)
    .join('\n');
  return error(errors[0]?.code ?? 'INVALID', message);
}

const docArg = { type: 'string', description: 'Document key from jinshu_list.' };

/**
 * Jinshu's five agent tools over the app's documents, for `lx.agent.provide` at launch. The caller's
 * identity comes from the call, never from arguments; each tool states its effect per call.
 */
export function createJinshuTools(options: JinshuToolsOptions): AgentTool[] {
  const { documents } = options;
  const exported = exportAiSchema(options.registry, { filePolicy: options.filePolicy });

  const openDoc = async (args: Args): Promise<JinshuSource | null> => {
    const key = typeof args.doc === 'string' ? args.doc : '';
    return key ? documents.open(key) : null;
  };
  const notFound = (args: Args) => error('NOT_FOUND', `No document "${String(args.doc ?? '')}". Call jinshu_list to find one.`);
  const titleOf = (source: JinshuSource | null, language: string) =>
    source?.getState().doc.title || logicText(language, '@jinshu.agent.untitled');
  const pageOf = (source: JinshuSource, at?: string) => {
    const top = at ? topLevelOf(source.getState().doc, at) : undefined;
    return documents.pageOf?.(source.key, top);
  };

  const list: AgentTool = {
    name: 'jinshu_list',
    summary: 'Find Jinshu documents by keyword.',
    description:
      'Search the app\'s Jinshu documents. Returns at most 20 rows: key, title, page, and whether the document is on screen, has unsaved changes, or has items waiting for the person.',
    inputSchema: {
      type: 'object',
      properties: { query: { type: 'string', description: 'Words in the title; empty lists recent documents.' } },
    },
    effect: 'read',
    async run(args) {
      const query = typeof args.query === 'string' ? args.query : '';
      const entries = await documents.list(query);
      const rows = entries.slice(0, LIST_LIMIT).map(entry => {
        const state = documents.peek?.(entry.key)?.getState();
        const page = documents.pageOf?.(entry.key);
        return {
          key: entry.key,
          title: entry.title,
          onScreen: (state?.viewers ?? 0) > 0,
          dirty: state?.dirty ?? false,
          waiting: state ? state.suggestions.length + state.conflicts.length + (state.pendingAgentSave ? 1 : 0) : 0,
          ...(page ? { page } : {}),
        };
      });
      return { status: 'ok', data: { documents: rows, ...(entries.length > LIST_LIMIT ? { more: true } : {}) } };
    },
  };

  const read: AgentTool = {
    name: 'jinshu_read',
    summary: 'Read a Jinshu document as markup with block ids.',
    description: `Read a Jinshu document as concise Jinshu Markup with stable block IDs: whole, as an outline, a range, a search, or what changed since a revision. With \`since\` and \`waitMs\` it waits (at most 25 s) until something changes or the person reviews something, then returns at once. The result lists review outcomes after \`since\` (suggestions accepted or rejected, save decisions, Stop, and your direct changes the person undid or redid, by \`batchId\`), the suggestions still waiting, whether a save request waits, and the blocks under the person's caret (\`focusIds\`). A read longer than 32,000 characters returns the outline; ask for a \`range\` or \`search\`.\n\n${exported.markupGuide}`,
    inputSchema: {
      type: 'object',
      required: ['doc'],
      properties: {
        doc: docArg,
        since: { type: 'integer', description: 'Return only blocks changed and reviews recorded after this revision.' },
        waitMs: { type: 'integer', minimum: 0, maximum: MAX_WAIT_MS, description: 'With `since`: wait up to this long for a change or review.' },
        outline: { type: 'boolean', description: 'Return only the heading outline.' },
        range: {
          type: 'object',
          properties: { from: { type: 'string' }, to: { type: 'string' } },
          description: 'Read a slice of top-level blocks by inclusive block IDs.',
        },
        search: { type: 'string', description: 'Return blocks matching this keyword plus 1 neighbor on each side.' },
      },
    },
    effect: 'read',
    async run(args, call) {
      const source = await openDoc(args);
      if (!source) return notFound(args);
      const origin = originOf(call);
      const since = typeof args.since === 'number' ? args.since : undefined;
      const waitMs = typeof args.waitMs === 'number' ? Math.min(Math.max(args.waitMs, 0), MAX_WAIT_MS) : 0;
      if (since !== undefined && waitMs > 0) {
        await source.waitForChange(since, anySignal(call.signal, timeoutSignal(waitMs)));
      }
      const range = args.range as { from?: string; to?: string } | undefined;
      if (since === undefined) {
        const blocks = source.getState().doc.blocks;
        const from = range?.from ? blocks.findIndex(b => b.id === range.from) : 0;
        const to = range?.to ? blocks.findIndex(b => b.id === range.to) : blocks.length - 1;
        const blockIds = range && from >= 0 && to >= from ? blocks.slice(from, to + 1).map(b => b.id) : [];
        source.setActivity({ id: newBlockId('act'), origin, kind: 'reading', blockIds });
      }
      const result = source.forAgent({
        since,
        outline: args.outline === true,
        range,
        search: typeof args.search === 'string' ? args.search : undefined,
        origin,
      });
      const page = pageOf(source);
      return { status: 'ok', data: { ...result, ...(page ? { page } : {}) } };
    },
  };

  const edit: AgentTool = {
    name: 'jinshu_edit',
    summary: 'Edit a Jinshu document by block id, directly or as suggestions.',
    description:
      'Apply an atomic batch of JinshuOp edits by block ID. `direct` changes the draft at once; `suggest` waits for the person to accept in the document. Direct edits to the block the person is typing in, rewrites and large deletions become suggestions. A suggestion returns status pending-review with its page; review outcomes come back from jinshu_read({ since }). STALE_BLOCK means the block changed after `base`: read it again and retry.',
    inputSchema: {
      type: 'object',
      required: ['doc', 'base', 'ops'],
      properties: {
        doc: docArg,
        base: { type: 'integer', description: 'Document revision from your last jinshu_read.' },
        mode: { type: 'string', enum: ['direct', 'suggest'], description: 'direct edits the draft now; suggest waits for review.' },
        ops: exported.operationsSchema,
      },
    },
    async effect(args, call): Promise<AgentAssessment> {
      const source = await openDoc(args);
      const title = titleOf(source, call.language);
      if (args.mode === 'suggest') return { effect: 'read', summary: logicText(call.language, '@jinshu.agent.suggest', { title }) };
      const count = Array.isArray(args.ops) ? args.ops.length : 0;
      return { effect: 'write', summary: logicText(call.language, '@jinshu.agent.edit', { title, count }) };
    },
    async run(args, call) {
      const source = await openDoc(args);
      if (!source) return notFound(args);
      // Without the revision the agent read, a stale edit would silently land on newer text.
      if (typeof args.base !== 'number' || !Number.isInteger(args.base)) {
        return error('INVALID', 'jinshu_edit needs "base": the revision from your last jinshu_read.');
      }
      const origin = originOf(call);
      const mode = args.mode === 'suggest' ? 'suggest' : 'direct';
      const ops = Array.isArray(args.ops) ? (args.ops as JinshuOp[]) : [];
      const res = source.apply({ batchId: newBlockId('ag'), origin, base: args.base, mode, ops });
      if (res.problems.some(p => p.severity === 'error')) return problemsResult(res.problems);
      const touched = source.getState().history.find(h => h.batchId === res.batchId)?.blockIds ?? [];
      const page = pageOf(source, touched[0] ?? opTarget(ops[0]));
      if (res.suggestionId) {
        const downgraded = source.getState().suggestions.find(s => s.id === res.suggestionId)?.downgraded;
        return {
          status: 'pending-review',
          summary: logicText(call.language, '@jinshu.agent.suggested', { title: titleOf(source, call.language) }),
          ...(page ? { page } : {}),
          data: { suggestionId: res.suggestionId, revision: res.revision, ...(downgraded ? { downgraded } : {}) },
        };
      }
      // Lingers briefly so a run of edits keeps a Stop in reach between calls.
      source.setActivity({ id: newBlockId('act'), origin, kind: 'writing', blockIds: touched });
      return { status: 'ok', data: { revision: res.revision, batchId: res.batchId, problems: res.problems, ...(page ? { page } : {}) } };
    },
  };

  const undo: AgentTool = {
    name: 'jinshu_undo',
    summary: 'Undo your last Jinshu edit, or withdraw your suggestion.',
    description:
      'Undo your own previous batch (or a specific batchId). Given the id of your pending suggestion, withdraws it. Refused when someone changed the same blocks later.',
    inputSchema: {
      type: 'object',
      required: ['doc'],
      properties: {
        doc: docArg,
        batchId: { type: 'string', description: 'A batchId or suggestionId of yours; default your last batch.' },
      },
    },
    async effect(args, call) {
      const source = await openDoc(args);
      return { effect: 'write', summary: logicText(call.language, '@jinshu.agent.undo', { title: titleOf(source, call.language) }) };
    },
    async run(args, call) {
      const source = await openDoc(args);
      if (!source) return notFound(args);
      const origin = originOf(call);
      const batchId = typeof args.batchId === 'string' ? args.batchId : undefined;
      const own = batchId && source.getState().suggestions.some(s => s.id === batchId && s.origin.id === origin.id);
      const res = own ? source.rejectSuggestion(batchId) : source.undo(batchId, origin);
      if (res.problems.some(p => p.severity === 'error')) return problemsResult(res.problems);
      return { status: 'ok', data: { revision: res.revision } };
    },
  };

  const save: AgentTool = {
    name: 'jinshu_save',
    summary: 'Ask to save a Jinshu document.',
    description:
      'Ask to save the draft. By the app\'s policy it saves, waits for the person to confirm in the document (status pending-review; the outcome comes back from jinshu_read({ since })), or is refused. Refused while suggestions, conflicts or uploads are pending.',
    inputSchema: {
      type: 'object',
      required: ['doc'],
      properties: {
        doc: docArg,
        summary: { type: 'string', description: 'What changed, shown to the person when they confirm.' },
      },
    },
    async effect(args, call) {
      const source = await openDoc(args);
      const summary = logicText(call.language, '@jinshu.agent.save', { title: titleOf(source, call.language) });
      // `confirm` asks the person in the document, so no consent card; `deny` refuses in `run`.
      return { effect: source?.agentSave === 'allow' ? 'write' : 'read', summary };
    },
    async run(args, call) {
      const source = await openDoc(args);
      if (!source) return notFound(args);
      const res = await source.requestSave(originOf(call), typeof args.summary === 'string' ? args.summary : undefined);
      if (res.pendingConfirmation) {
        const page = pageOf(source);
        return {
          status: 'pending-review',
          summary: logicText(call.language, '@jinshu.agent.saveWaiting', { title: titleOf(source, call.language) }),
          ...(page ? { page } : {}),
          data: { revision: source.getState().revision },
        };
      }
      if (!res.saved) return problemsResult(res.problems);
      return { status: 'ok', data: { saved: true, revision: source.getState().revision } };
    },
  };

  return [list, read, edit, undo, save];
}

/** The top-level block holding `id`, or `id` itself at the top. */
function topLevelOf(doc: JinshuDocument, id: string): string | undefined {
  for (const top of doc.blocks) {
    let found = false;
    walkBlocks([top], block => {
      if (block.id === id) found = true;
    });
    if (found) return top.id;
  }
  return undefined;
}

/** The block an op points at, for a suggestion's page. */
function opTarget(op: JinshuOp | undefined): string | undefined {
  if (!op) return undefined;
  const o = op as unknown as { id?: string; target?: { blockId?: string }; at?: Record<string, string> };
  return o.id ?? o.target?.blockId ?? o.at?.after ?? o.at?.before;
}
