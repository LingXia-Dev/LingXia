import { definePage, lx } from '@lingxia/logic';
import { extractOutline, type JinshuDocument, type JinshuProgress, type Observation, type OutlineItem } from '@liulidesign/jinshu/schema';
import type { MinimalChannelEndpoint } from '@liulidesign/jinshu/source';
import {
  getSavedDoc,
  isCourse,
  LESSON_KEY,
  markSeen,
  loadProgress,
  observe,
  openAgentPage,
  openSource,
  pickScenario,
  runScenario,
  serveDoc,
  sourceFile,
  takeImportNotes,
  watchRuns,
  type RunLog,
  type ScenarioId,
  type SourceFile,
} from '../../shared/store';
import { headOf, type ShownFile } from '../../shared/agent';

export type DocMode = 'read' | 'edit' | 'changes';

/** The file as the page shows it: a long one is cut, and says how much more there is. */
function clipFile(file: SourceFile): ShownFile {
  const head = headOf(file.text);
  if (file.format === 'markdown') return { ...file, text: head.text, more: head.more };
  const md = headOf(file.markdown);
  return { ...file, text: head.text, more: head.more, markdown: md.text, markdownMore: md.more };
}

export default definePage({
  data: {
    /** Empty until onLoad: nothing connects to a document before the page knows which, and how. */
    docKey: '',
    title: '',
    isCourse: false,
    mode: 'read' as DocMode,
    dirty: false,
    /** Suggestions, a save request and conflicts waiting for the person. */
    waiting: 0,
    saveStatus: 'saved' as 'saved' | 'dirty' | 'saving' | 'failed',
    revision: 0,
    outline: [] as OutlineItem[],
    resume: undefined as JinshuProgress | undefined,
    // Sent only while the page shows them: a long document is large.
    savedDoc: null as JinshuDocument | null,
    draftDoc: null as JinshuDocument | null,
    runs: [] as RunLog[],
    /** The block to open at: where an agent's change landed. */
    openAt: '',
    format: 'json' as 'json' | 'markdown',
    /** The file as `save` would write it, while the person looks at it. */
    file: null as ShownFile | null,
    /** What a just-imported Markdown text could not keep, shown once. */
    importNotes: [] as string[],
  },
  setup(thisPage) {
    let key = LESSON_KEY;
    let mode: DocMode = 'read';
    let unsub = () => {};
    let unwatchRuns = () => {};

    const show = async () => {
      const source = await openSource(key);
      if (!source) return;
      const sync = () => {
        const s = source.getState();
        thisPage.update(d => {
          d.docKey = key;
          d.title = s.doc.title ?? '';
          d.isCourse = isCourse(key);
          d.format = source.format;
          d.mode = mode;
          d.dirty = s.dirty;
          d.waiting = s.suggestions.length + (s.pendingAgentSave ? 1 : 0) + s.conflicts.length;
          d.saveStatus = s.saveStatus;
          d.revision = s.revision;
          d.outline = extractOutline(s.doc);
          d.savedDoc = mode === 'changes' ? getSavedDoc(key) : null;
          d.draftDoc = mode === 'changes' ? s.doc : null;
        });
      };
      sync();
      unsub = source.subscribe(sync);
      return sync;
    };
    let resync: (() => void) | undefined;

    return {
      async onLoad(query?: { doc?: string; mode?: DocMode; at?: string }) {
        key = query?.doc ?? LESSON_KEY;
        mode = query?.mode ?? 'read';
        const notes = takeImportNotes(key);
        thisPage.update(d => {
          d.openAt = query?.at ?? '';
          d.importNotes = notes;
          // A page reused for another document never keeps the last one's file.
          d.file = null;
        });
        // Each page shows the runs started from it; another document's page starts clean.
        unwatchRuns = watchRuns(runs => thisPage.update(d => void (d.runs = runs.filter(r => r.doc === key))));
        const resume = isCourse(key) ? await loadProgress(key) : undefined;
        thisPage.update(d => void (d.resume = resume));
        resync = await show();
        markSeen(key);
      },
      onUnload() {
        // The page stops listening; the source and its draft stay for other pages and the agent.
        markSeen(key);
        unsub();
        unwatchRuns();
      },
      actions: {
        syncDoc(params: unknown, channel: MinimalChannelEndpoint) {
          serveDoc(params, channel);
        },
        setMode({ mode: next }: { mode: DocMode }) {
          mode = next;
          resync?.();
        },
        async showFile() {
          const file = await sourceFile(key);
          thisPage.update(d => void (d.file = file && clipFile(file)));
        },
        async copyFile({ as }: { as: 'file' | 'markdown' }) {
          const file = await sourceFile(key);
          if (!file) return;
          await lx.clipboard.writeText(as === 'markdown' && file.format === 'json' ? file.markdown : file.text);
          lx.ui.toast('已复制');
        },
        closeFile() {
          thisPage.update(d => void (d.file = null));
        },
        dismissNotes() {
          thisPage.update(d => void (d.importNotes = []));
        },
        observe({ events }: { events: readonly Observation[] }) {
          observe(key, events);
        },
        async copyText({ text }: { text: string }) {
          await lx.clipboard.writeText(text);
          lx.ui.toast('已复制');
        },
        run({ scenario }: { scenario: ScenarioId }) {
          void runScenario(scenario, key);
        },
        pick() {
          void pickScenario(key);
        },
        openPage({ page }: { page: string }) {
          return openAgentPage(page);
        },
        async back() {
          await lx.nav.back();
        },
      },
    };
  },
});
