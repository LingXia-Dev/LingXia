/** The agent drill and the document rows; shared by Logic and View. */

export type ScenarioId = 'read' | 'suggest' | 'direct' | 'focus' | 'save' | 'elsewhere' | 'outcomes' | 'stream' | 'conflict';

export interface Scenario {
  readonly id: ScenarioId;
  readonly title: string;
  /** What to watch in the document while it runs. */
  readonly watch: string;
  /** `tools`: an outside agent through the registered tools. `app`: the app's own code. */
  readonly via: 'tools' | 'app';
}

export const SCENARIOS: readonly Scenario[] = [
  { id: 'read', via: 'tools', title: '读文档', watch: '读到的是带块 id 的精简标记，不是 JSON。' },
  { id: 'suggest', via: 'tools', title: '提建议', watch: '建议留在原处，等你接受或拒绝。' },
  { id: 'direct', via: 'tools', title: '直接写入', watch: '写入处有标记；整批可撤销，撤销会告诉 Agent。' },
  { id: 'focus', via: 'tools', title: '改你正在写的段落', watch: '先进入编辑并点进一段：直接修改自动变成建议，不打断输入。' },
  { id: 'save', via: 'tools', title: '请求保存', watch: 'Agent 不能自己保存，要你确认。' },
  { id: 'elsewhere', via: 'tools', title: '改一篇没打开的文档', watch: '改动进入那篇的草稿；文档库标出「助手改过」。' },
  { id: 'outcomes', via: 'tools', title: '等你的决定', watch: 'Agent 挂起等待；你一审阅或撤销，结果立刻返回。' },
  { id: 'stream', via: 'app', title: '流式写入', watch: '应用自己调用模型时逐块写入，可随时停止。' },
  { id: 'conflict', via: 'app', title: '保存时服务器有新版本', watch: '按块合并；两边都改的块逐条由你决定。' },
];

/** One call and what came back, verbatim. */
export interface CallLog {
  readonly name: string;
  readonly args: string;
  readonly status: 'running' | 'ok' | 'pending-review' | 'error';
  readonly result?: string;
}

/** One scenario run. */
export interface RunLog {
  readonly id: string;
  readonly scenario: ScenarioId;
  /** The document the run was started from. */
  readonly doc: string;
  readonly calls: readonly CallLog[];
  readonly status: 'running' | 'done' | 'failed';
  /** What happened, in a sentence. */
  readonly outcome?: string;
  /** Another document's page, when the run changed one. */
  readonly page?: string;
  readonly pageTitle?: string;
}

/** One document in the library. */
export interface DocRow {
  readonly key: string;
  readonly title: string;
  readonly kind: string;
  /** How the document is stored. */
  readonly format: 'json' | 'markdown';
  /** Saved as the person edits, so there is no unsaved state to show. */
  readonly autoSave: boolean;
  readonly blocks: number;
  readonly dirty: boolean;
  readonly suggestions: number;
  readonly saveWaiting: boolean;
  /** An agent changed it since the person last had it open. */
  readonly agentChanged: boolean;
  /** Share of the document read, 0–1, when it is tracked. */
  readonly readRatio?: number;
}

/** A document's file as `save` would write it; a JSON one also as Markdown, with what Markdown cannot keep. */
export type SourceFile =
  | { readonly doc: string; readonly format: 'markdown'; readonly text: string }
  // `lost`: what Markdown cannot keep, as words for a person.
  | { readonly doc: string; readonly format: 'json'; readonly text: string; readonly markdown: string; readonly lost: readonly string[] };

/** A file as a page shows it; `more` counts the lines left out. */
export type ShownFile = SourceFile & { readonly more: number; readonly markdownMore?: number };

/** The lines a page shows of a file; copying takes the whole file from Logic. */
export const FILE_LINES = 400;

/** The first `FILE_LINES` lines of `text`, and how many more there are. */
export function headOf(text: string): { readonly text: string; readonly more: number } {
  const lines = text.split('\n');
  return lines.length <= FILE_LINES ? { text, more: 0 } : { text: lines.slice(0, FILE_LINES).join('\n'), more: lines.length - FILE_LINES };
}
