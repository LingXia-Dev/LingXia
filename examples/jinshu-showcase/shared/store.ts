import { lx } from '@lingxia/logic';
import {
  documentToMarkdown,
  markdownToDocument,
  plainText,
  reduceObservations,
  run,
  type JinshuDocument,
  type JinshuFormat,
  type JinshuProgress,
  type Observation,
} from '@liulidesign/jinshu/schema';
import {
  createJinshuSources,
  createJinshuTools,
  type AgentCall,
  type AgentResult,
  type ChannelParams,
  type JinshuDocumentEntry,
  type JinshuMediaAdapter,
  type JinshuSource,
  type MinimalChannelEndpoint,
  type PickResult,
  type SaveFile,
  formatProblemWords,
} from '@liulidesign/jinshu/source';
import { SCENARIOS, type CallLog, type DocRow, type RunLog, type ScenarioId } from './agent';
import { FIELD_NOTES_MD } from './fieldNotes';
import { demoRegistry, draftDoc, lessonDoc, longDoc, meetingDoc } from './fixtures';

export type { DocRow, RunLog, ScenarioId, SourceFile } from './agent';
import type { SourceFile } from './agent';

export const LESSON_KEY = 'light-lesson';
const LONG_KEY = 'photo-handbook';
export const NOTES_KEY = 'field-notes';

type CatalogueEntry = {
  readonly kind: string;
  /** Courseware: video seeks only into what was watched, at most 1.5×. */
  readonly isCourse?: boolean;
} & (
  | { readonly format?: 'json'; readonly doc: () => JinshuDocument }
  // Stored as Markdown text and saved automatically.
  | { readonly format: 'markdown'; readonly text: string }
);

/** The app's documents, in the library's order: the "server" copy of each. */
const CATALOGUE: Record<string, CatalogueEntry> = {
  [LESSON_KEY]: { kind: '课程', doc: () => lessonDoc, isCourse: true },
  'offline-reading-spec': { kind: '需求', doc: () => draftDoc },
  'weekly-notes': { kind: '会议纪要', doc: () => meetingDoc },
  [NOTES_KEY]: { kind: '笔记', format: 'markdown', text: FIELD_NOTES_MD },
  [LONG_KEY]: { kind: '手册', doc: longDoc },
};
/** The library's keys, imported documents first. */
export const docKeys = () => Object.keys(CATALOGUE).sort((a, b) => Number(b.startsWith('import-')) - Number(a.startsWith('import-')));
export const isCourse = (key: string) => CATALOGUE[key]?.isCourse === true;
const formatOf = (key: string): JinshuFormat => CATALOGUE[key]?.format ?? 'json';

/** The catalogue's copy of a document, read from its text for a Markdown one. */
function catalogueDoc(key: string): JinshuDocument {
  const entry = CATALOGUE[key];
  if (!entry) return lessonDoc;
  return entry.format === 'markdown' ? markdownToDocument(entry.text, { registry: demoRegistry }).doc : entry.doc();
}


type MediaKind = 'image' | 'video' | 'audio' | 'file';

/** This app's own asset library: what an author may insert without uploading anything. */
const ASSET_LIBRARY: Record<MediaKind, PickResult> = {
  image: { src: 'public/photos/photo-4.png', width: 480, height: 360 },
  video: { src: 'public/media/lesson-clip.mp4', poster: 'public/photos/photo-5.png' },
  audio: { src: 'public/media/jinshu-intro.m4a', name: '课程导读', durationMs: 8_400 },
  file: { src: './slides/jinshu-arch.pdf', name: 'jinshu-architecture.pdf', mimeType: 'application/pdf', sizeBytes: 2_450_000 },
};

const DEVICE_LABEL: Record<MediaKind, string> = { image: '从相册选择', video: '从相册选择', audio: '从设备选择', file: '从设备选择' };

/** The app decides where media comes from: its asset library or the device. */
const media: JinshuMediaAdapter = {
  async pick({ kind }) {
    const choice = await lx.ui.actionSheet({
      items: [
        { id: 'library', label: '素材库' },
        { id: 'device', label: DEVICE_LABEL[kind] },
      ],
    });
    if (choice.status !== 'ok') return null;
    if (choice.id === 'library') return ASSET_LIBRARY[kind];
    const picked = kind === 'image' || kind === 'video' ? await lx.pick.media({ types: [kind], limit: 1 }) : await lx.pick.file();
    if (picked.status !== 'ok') return null;
    return { file: 'files' in picked ? picked.files[0] : picked.file };
  },
  // Stands in for the app's upload service; the block shows the local file until it resolves.
  async upload(file, onProgress) {
    for (const ratio of [0.2, 0.45, 0.7, 0.9]) {
      await pause(350);
      onProgress(ratio);
    }
    return { src: file.path, mimeType: file.type, sizeBytes: file.size };
  },
};

// The "server": each saved document, and for a Markdown one the file text exactly as written.
const saved: Record<string, { doc: JinshuDocument; version: number; text?: string }> = {};
/** The next save of these documents finds a colleague's newer version on the "server". */
const conflictNext: Record<string, boolean> = {};

/** A colleague's version: the saved copy with its first paragraph reworded. */
function colleagueVersion(key: string): JinshuDocument {
  const base = getSavedDoc(key);
  const at = base.blocks.findIndex(b => b.type === 'paragraph');
  const blocks = base.blocks.map((block, i) => (i === at && block.type === 'paragraph' ? { ...block, runs: [run(`（同事修订）${plainText(block.runs)}`)] } : block));
  return { ...base, blocks };
}

/**
 * One source per document for as long as Logic runs. Pages and the agent tools open the same key,
 * so an agent's change shows live wherever the document is open and waits in the draft otherwise.
 */
export const sources = createJinshuSources(key => {
  const entry = CATALOGUE[key];
  if (!entry) return null;
  return {
    registry: demoRegistry,
    ...(entry.format === 'markdown' ? { format: 'markdown', initialText: entry.text, saveMode: 'auto' } as const : { initialDoc: entry.doc() }),
    initialVersion: 'v1',
    filePolicy: { default: 'attachment', inline: 'never' },
    agentSave: 'confirm',
    media,
    // Crash recovery: an unsaved draft and its pending suggestions survive a kill.
    storage: {
      get: k => lx.storage.get(k),
      set: (k, value) => lx.storage.set(k, value as never),
      remove: k => lx.storage.delete(k),
    },
    async save(next, _base, file) {
      const version = (saved[key]?.version ?? 1) + 1;
      if (conflictNext[key]) {
        conflictNext[key] = false;
        const remoteDoc = colleagueVersion(key);
        saved[key] = { doc: remoteDoc, version };
        return { ok: false, conflict: true, remoteDoc, remoteVersion: `v${version}` };
      }
      saved[key] = { doc: next, version, ...(entry.format === 'markdown' ? { text: file.text } : {}) };
      return { ok: true, version: `v${version}` };
    },
  };
});

/** A document's page; `at` opens it in the editor at that block, where the change is reviewed. */
export const pageOf = (key: string, at?: string) => (at ? `doc?doc=${key}&mode=edit&at=${at}` : `doc?doc=${key}`);

/** The tools a host agent reaches; the app registers them once at launch, never per page. */
export const agentTools = createJinshuTools({
  documents: {
    list: query =>
      docKeys().map(key => ({ key, title: titleOf(key) }))
        .filter(row => !query || row.title.includes(query))
        .map((row): JinshuDocumentEntry => row),
    open: key => sources.open(key),
    peek: key => sources.peek(key),
    pageOf,
  },
  registry: demoRegistry,
  filePolicy: { default: 'attachment', inline: 'never' },
});

function titleOf(key: string): string {
  return sources.peek(key)?.getState().doc.title ?? catalogueDoc(key).title ?? key;
}

export function getSavedDoc(key: string): JinshuDocument {
  return saved[key]?.doc ?? catalogueDoc(key);
}

/** The Markdown file the "server" holds for a document, as last saved. */
export function getSavedText(key: string): string | undefined {
  const entry = CATALOGUE[key];
  return saved[key]?.text ?? (entry?.format === 'markdown' ? entry.text : undefined);
}

/** The draft as `save` would write it now; a JSON document also as Markdown, with what Markdown cannot keep. */
export async function sourceFile(key: string): Promise<SourceFile | null> {
  const source = await openSource(key);
  if (!source) return null;
  const file: SaveFile = source.file();
  if (source.format === 'markdown') return { doc: key, format: 'markdown', text: file.text };
  const exported = documentToMarkdown(source.getState().doc);
  return { doc: key, format: 'json', text: file.text, markdown: exported.text, lost: formatProblemWords(exported.problems, lx.app.language.get()) };
}

let importSeq = 0;
/** What reading a just-imported text changed, as words, shown once when it opens. */
const importNotes: Record<string, string[]> = {};

/** A new Markdown document from pasted text; returns its key and what parsing could not keep. */
export async function importMarkdown(text: string): Promise<{ key: string; warnings: readonly string[] }> {
  const key = `import-${++importSeq}`;
  CATALOGUE[key] = { kind: '导入', format: 'markdown', text };
  const source = await sources.open(key);
  source?.subscribe(() => notifyLibrary());
  const warnings = formatProblemWords(source?.getState().problems ?? [], lx.app.language.get());
  if (warnings.length > 0) importNotes[key] = warnings;
  notifyLibrary();
  return { key, warnings };
}

/** The import warnings for a document, once: a later open shows none. */
export function takeImportNotes(key: string): string[] {
  const notes = importNotes[key] ?? [];
  delete importNotes[key];
  return notes;
}


// ---------------------------------------------------------------------------------------------
// What the app tracks around its documents: reading progress, and what the person has seen.

const progressKey = (key: string) => `jinshu-showcase:progress:${key}`;
const progress: Record<string, JinshuProgress> = {};
/** The revision each document was at when the person last had it open. */
const seenRevision: Record<string, number> = {};

export async function loadProgress(key: string): Promise<JinshuProgress | undefined> {
  if (!progress[key]) {
    const stored = await lx.storage.get(progressKey(key));
    if (stored) progress[key] = stored as unknown as JinshuProgress;
  }
  return progress[key];
}

export function observe(key: string, events: readonly Observation[]): void {
  progress[key] = reduceObservations(progress[key], events);
  void lx.storage.set(progressKey(key), progress[key] as never);
  notifyLibrary();
}

export function markSeen(key: string): void {
  const source = sources.peek(key);
  if (source) seenRevision[key] = source.getState().revision;
  notifyLibrary();
}

function readRatio(key: string, doc: JinshuDocument): number | undefined {
  const p = progress[key];
  if (!p?.position) return undefined;
  const at = doc.blocks.findIndex(b => b.id === p.position!.blockId);
  return at < 0 ? undefined : (at + 1) / doc.blocks.length;
}

export function libraryRows(): DocRow[] {
  return docKeys().map(key => {
    const state = sources.peek(key)?.getState();
    const doc = state?.doc ?? catalogueDoc(key);
    const seen = seenRevision[key] ?? 0;
    return {
      key,
      title: doc.title ?? key,
      kind: CATALOGUE[key].kind,
      format: formatOf(key),
      autoSave: formatOf(key) === 'markdown',
      blocks: doc.blocks.length,
      dirty: state?.dirty ?? false,
      suggestions: state?.suggestions.length ?? 0,
      saveWaiting: state?.pendingAgentSave !== undefined,
      agentChanged: (state?.history ?? []).some(h => h.origin.kind === 'agent' && !h.undone && h.revision > seen),
      readRatio: readRatio(key, doc),
    };
  });
}

const libraryListeners = new Set<() => void>();
function notifyLibrary(): void {
  for (const fn of libraryListeners) fn();
}

/** Every document, opened once at launch so the library shows live state. */
const opened = Promise.all(docKeys().map(key => sources.open(key))).then(all => {
  for (const source of all) source?.subscribe(() => notifyLibrary());
  return all;
});

export function watchLibrary(fn: () => void): () => void {
  libraryListeners.add(fn);
  void opened.then(fn);
  return () => libraryListeners.delete(fn);
}

export async function openSource(key: string): Promise<JinshuSource | null> {
  await opened;
  return sources.open(key);
}

/** A page's `syncDoc`: serves the channel of the document the page asked for. */
export function serveDoc(params: unknown, channel: MinimalChannelEndpoint): void {
  const opened = params as ChannelParams | undefined;
  const key = opened?.doc;
  void sources.open(typeof key === 'string' ? key : LESSON_KEY).then(source => source?.serve(channel, opened));
}

// ---------------------------------------------------------------------------------------------
// The agent drill. An outside agent is not part of Jinshu or of this app: in a real host it reaches
// the tools registered at launch. Here each scenario makes the calls a model would, and logs them.

const AGENT = { appId: 'demo-host', name: '宿主' };
let runs: RunLog[] = [];
let runSeq = 0;
const lastRevision: Record<string, number> = {};
const runListeners = new Set<(runs: readonly RunLog[]) => void>();

export function watchRuns(fn: (runs: readonly RunLog[]) => void): () => void {
  runListeners.add(fn);
  fn(runs);
  return () => runListeners.delete(fn);
}

function publish(): void {
  for (const fn of runListeners) fn(runs);
}

export function clearRuns(): void {
  runs = runs.filter(r => r.status === 'running');
  publish();
}

const pause = (ms: number) => new Promise<void>(resolve => setTimeout(() => resolve(), ms));
const MAX_RESULT = 2400;

function show(value: unknown): string {
  if (value === undefined) return '';
  // A read's markup is the point: print it as text, the rest as JSON beside it.
  if (value && typeof value === 'object' && 'view' in value) {
    const { view, ...rest } = value as { view: string };
    const tail = JSON.stringify(rest);
    return clip(`${view}\n\n${tail}`);
  }
  return clip(JSON.stringify(value, null, 1));
}

const clip = (text: string) => (text.length > MAX_RESULT ? `${text.slice(0, MAX_RESULT)}\n…（共 ${text.length} 字）` : text);

class Run {
  private log: RunLog;
  constructor(scenario: ScenarioId, doc: string) {
    this.log = { id: `r${++runSeq}`, scenario, doc, calls: [], status: 'running' };
    runs = [...runs, this.log].slice(-12);
    publish();
  }
  private set(patch: Partial<RunLog>) {
    const id = this.log.id;
    this.log = { ...this.log, ...patch };
    runs = runs.map(r => (r.id === id ? this.log : r));
    publish();
  }
  private push(call: CallLog): number {
    this.set({ calls: [...this.log.calls, call] });
    return this.log.calls.length - 1;
  }
  private patch(at: number, call: Partial<CallLog>) {
    this.set({ calls: this.log.calls.map((c, i) => (i === at ? { ...c, ...call } : c)) });
  }
  /** One tool call, exactly as a host would make it. */
  async tool(name: string, args: Record<string, unknown>): Promise<AgentResult> {
    const tool = agentTools.find(t => t.name === name)!;
    const at = this.push({ name, args: JSON.stringify(args), status: 'running' });
    const call: AgentCall = { caller: AGENT, conversation: 'drill', agent: '助手', approved: true, language: 'zh-CN', signal: new AbortController().signal };
    const result = await tool.run(args, call);
    this.patch(at, {
      status: result.status,
      result: result.status === 'error' ? `${result.code}: ${result.message}` : result.status === 'pending-review' ? show({ summary: result.summary, ...((result.data as object) ?? {}) }) : show(result.data),
    });
    return result;
  }
  /** A call into the app's own Logic API, logged the same way. */
  app(name: string, args: string, result?: unknown): number {
    return this.push({ name, args, status: 'ok', result: show(result) });
  }
  appResult(at: number, result: unknown, status: CallLog['status'] = 'ok') {
    this.patch(at, { result: show(result), status });
  }
  done(outcome: string, page?: { page: string; title: string }) {
    this.set({ status: 'done', outcome, ...(page ? { page: page.page, pageTitle: page.title } : {}) });
  }
  fail(outcome: string) {
    this.set({ status: 'failed', outcome });
  }
}

const failure = (result: AgentResult) =>
  result.status === 'error' ? (result.code === 'STOPPED' ? '你停止了 Agent；它要重新读取文档才能再改。' : `没有改成：${result.message.split('\n')[0]}`) : '';

async function revisionOf(run: Run, doc: string): Promise<number> {
  const read = await run.tool('jinshu_read', { doc, outline: true });
  const revision = read.status === 'ok' ? (read.data as { revision: number }).revision : 1;
  lastRevision[doc] = revision;
  return revision;
}

// A model would pick the place from the markup it read; the drill picks the first fitting clause.
function clauseIn(key: string, blockId?: string): { id: string; find: string } | null {
  const doc = sources.peek(key)?.getState().doc;
  for (const block of doc?.blocks ?? []) {
    if (blockId ? block.id !== blockId : block.type !== 'paragraph') continue;
    if (!('runs' in block) || !Array.isArray(block.runs)) continue;
    const text = plainText(block.runs as never);
    const clause = text.split(/[，。；：！？]/)[0];
    if (clause.length >= 2 && clause.length <= 30) return { id: block.id, find: clause };
    if (blockId && text.length >= 2) return { id: block.id, find: text.slice(0, 12) };
  }
  return null;
}

export async function runScenario(scenario: ScenarioId, doc: string): Promise<void> {
  const run = new Run(scenario, doc);
  try {
    await drill(scenario, doc, run);
  } catch (error) {
    run.fail(`出错：${String(error)}`);
  }
}

async function drill(scenario: ScenarioId, doc: string, run: Run): Promise<void> {
  if (scenario === 'read') {
    const result = await run.tool('jinshu_read', { doc });
    if (result.status !== 'ok') return run.fail(failure(result));
    lastRevision[doc] = (result.data as { revision: number }).revision;
    return run.done('这就是模型拿到的全部上下文：结构、块 id 和文字，没有排版细节。');
  }

  if (scenario === 'suggest') {
    const base = await revisionOf(run, doc);
    const target = clauseIn(doc);
    if (!target) return run.fail('这篇没有适合改写的短句。');
    const result = await run.tool('jinshu_edit', {
      doc,
      base,
      mode: 'suggest',
      ops: [{ op: 'edit', id: target.id, find: target.find, replace: `<b>${target.find}</b>`, note: '加粗要点' }],
    });
    if (result.status === 'error') return run.fail(failure(result));
    return run.done(`建议把「${target.find}」加粗。正文没有变，建议在编辑模式里等你决定。`);
  }

  if (scenario === 'direct') {
    const base = await revisionOf(run, doc);
    const first = sources.peek(doc)?.getState().doc.blocks[0];
    const result = await run.tool('jinshu_edit', {
      doc,
      base,
      ops: [
        {
          op: 'insert',
          at: first ? { after: first.id } : { end: 'doc' },
          markup: '<callout tone="info"><title>摘要</title><p>先读第二节的表格，再回到第一节的示例，理解会快很多。</p></callout>',
          note: '补一段摘要',
        },
      ],
    });
    if (result.status === 'error') return run.fail(failure(result));
    return run.done('摘要已写进草稿，标记在开头。觉得不合适，在编辑模式里撤销这次修改；用「等你的决定」看 Agent 收到什么。');
  }

  if (scenario === 'focus') {
    const focus = sources.peek(doc)?.getState().focusIds ?? [];
    const target = focus.length > 0 ? clauseIn(doc, focus[0]) : null;
    if (!target) return run.fail('没有正在编辑的段落：先切到「编辑」，点进一段文字，再运行。');
    const base = await revisionOf(run, doc);
    const result = await run.tool('jinshu_edit', {
      doc,
      base,
      ops: [{ op: 'edit', id: target.id, find: target.find, replace: `<b>${target.find}</b>`, note: '加粗要点' }],
    });
    if (result.status === 'error') return run.fail(failure(result));
    return run.done('Agent 要求直接修改，但这一段你正在写：锦书把它改成了建议，你的输入没有被打断。');
  }

  if (scenario === 'save') {
    const result = await run.tool('jinshu_save', { doc, summary: '整理完成' });
    if (result.status === 'error') return run.fail(failure(result));
    if (result.status === 'pending-review') return run.done('保存请求在文档里等你确认；确认前服务器上的版本不变。');
    return run.done('已保存。');
  }

  if (scenario === 'elsewhere') {
    const list = await run.tool('jinshu_list', {});
    const rows = list.status === 'ok' ? (list.data as { documents: { key: string; title: string; onScreen?: boolean }[] }).documents : [];
    const target = rows.find(row => row.key !== doc && !row.onScreen && row.key !== LONG_KEY) ?? rows.find(row => row.key !== doc);
    if (!target) return run.fail('没有别的文档。');
    const base = await revisionOf(run, target.key);
    const result = await run.tool('jinshu_edit', {
      doc: target.key,
      base,
      ops: [{ op: 'insert', at: { end: 'doc' }, markup: '<p>补记：下次讨论前，请各自把意见写在对应段落下面。</p>', note: '补记' }],
    });
    if (result.status === 'error') return run.fail(failure(result));
    const page = (result.data as { page?: string }).page ?? pageOf(target.key);
    return run.done(`《${target.title}》没有打开，改动进了它的草稿；文档库里它带着「助手改过」。打开时直接落在改动处。`, { page, title: target.title });
  }

  if (scenario === 'outcomes') {
    const since = lastRevision[doc] ?? 1;
    const result = await run.tool('jinshu_read', { doc, since, waitMs: 20_000, outline: true });
    if (result.status !== 'ok') return run.fail(failure(result));
    const data = result.data as { revision: number; reviews: { kind: string; accepted?: boolean }[]; pending: string[]; saveWaiting: boolean };
    lastRevision[doc] = data.revision;
    const accepted = data.reviews.filter(r => r.kind === 'suggestion' && r.accepted === true).length;
    const rejected = data.reviews.filter(r => r.kind === 'suggestion' && r.accepted === false).length;
    const undone = data.reviews.filter(r => r.kind === 'undone').length - data.reviews.filter(r => r.kind === 'redone').length;
    const saved = data.reviews.find(r => r.kind === 'save');
    const facts = [
      accepted > 0 ? `接受了 ${accepted} 条建议` : '',
      rejected > 0 ? `拒绝了 ${rejected} 条` : '',
      undone > 0 ? `撤销了 ${undone} 次直接修改` : '',
      saved ? (saved.accepted ? '确认了保存' : '拒绝了保存') : '',
    ].filter(Boolean);
    const waiting = data.pending.length + (data.saveWaiting ? 1 : 0);
    if (facts.length === 0) return run.done(waiting > 0 ? `20 秒内没有新决定，还有 ${waiting} 项在等你。` : '20 秒内没有新决定。');
    return run.done(`Agent 收到：你${facts.join('，')}。它据此决定下一步，不会把撤销的内容改回去。`);
  }

  const source = await sources.open(doc);
  if (!source) return run.fail('文档打不开。');

  if (scenario === 'stream') {
    const parts = [
      '<h2>小结</h2>',
      '<p>光的方向决定质感，色温决定情绪。</p>',
      '<p>出门前查好时间，到了现场先看影子。</p>',
      '<p>回来对照这一课的表格，挑出最满意的一张。</p>',
    ];
    const origin = { kind: 'agent', id: 'demo-app-model', name: '应用的模型' } as const;
    const at = run.app('source.stream', "{ at: { end: 'doc' } }");
    const stream = source.stream({ at: { end: 'doc' }, origin });
    for (const part of parts) {
      for (let i = 0; i < part.length; i += 6) {
        stream.push(part.slice(i, i + 6));
        await pause(60);
        if (!source.getState().activities.some(a => a.streamId === stream.streamId)) {
          run.appResult(at, { aborted: true }, 'error');
          return run.done('你停止了写入；已经写完的块留在草稿里，写到一半的丢弃。');
        }
      }
    }
    const finished = stream.finish();
    run.appResult(at, finished);
    return run.done('四块逐块出现在文末，每块写完才定型；写的时候页面上有停止按钮。');
  }

  // conflict
  if (!source.getState().dirty) return run.fail('草稿和服务器一致，没有可保存的：先改点什么，再运行。');
  conflictNext[doc] = true;
  const at = run.app('source.save', '()');
  const result = await source.save();
  run.appResult(at, result, result.ok ? 'ok' : 'error');
  const conflicts = source.getState().conflicts.length;
  return run.done(
    conflicts > 0
      ? `同事先保存了一版。不冲突的块已经合并，${conflicts} 处两边都改了，在编辑模式里逐条决定。`
      : '同事先保存了一版，改的是别的块，已自动合并进你的草稿；再保存一次即可。',
  );
}

/** Opens a document's page from a tool result's `page` (`doc?doc=…&mode=…&at=…`). */
export async function openAgentPage(page: string): Promise<void> {
  const query = new Map(page.slice(page.indexOf('?') + 1).split('&').map(pair => pair.split('=') as [string, string]));
  const doc = query.get('doc');
  if (!doc) return;
  const params: Record<string, string> = { doc };
  for (const key of ['mode', 'at']) if (query.get(key)) params[key] = query.get(key)!;
  await lx.nav.go({ page: 'doc', params }).opened;
}

/** The phone's scenario picker: a system sheet, so nothing covers the document while it runs. */
export async function pickScenario(doc: string): Promise<void> {
  const choice = await lx.ui.actionSheet({ items: SCENARIOS.map(s => ({ id: s.id, label: s.title })) });
  if (choice.status === 'ok') await runScenario(choice.id as ScenarioId, doc);
}
