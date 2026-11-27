import type { View } from '@lingxia/view';
import type { JinshuDocument, JinshuProgress, Observation, OutlineItem } from '@liulidesign/jinshu/schema';
import type { JinshuChannelAction } from '@liulidesign/jinshu/JinshuView';
import { Workspace, type DocMode } from '../../content/workspace';
import type { RunLog, ScenarioId, ShownFile } from '../../shared/agent';

type Actions = {
  syncDoc: JinshuChannelAction;
  setMode(args: { mode: DocMode }): void;
  observe(args: { events: readonly Observation[] }): void;
  showFile(): Promise<void>;
  copyFile(args: { as: 'file' | 'markdown' }): Promise<void>;
  closeFile(): void;
  dismissNotes(): void;
  copyText(args: { text: string }): Promise<void>;
  run(args: { scenario: ScenarioId }): void;
  pick(): void;
  openPage(args: { page: string }): Promise<void>;
  back(): Promise<void>;
};

type Data = {
  docKey: string;
  mode: DocMode;
  isCourse: boolean;
  dirty: boolean;
  waiting: number;
  saveStatus: 'saved' | 'dirty' | 'saving' | 'failed';
  revision: number;
  outline: OutlineItem[];
  resume?: JinshuProgress;
  savedDoc: JinshuDocument | null;
  draftDoc: JinshuDocument | null;
  runs: RunLog[];
  openAt: string;
  format: 'json' | 'markdown';
  file: ShownFile | null;
  importNotes: string[];
};

export function render({ data, actions }: View<Actions, Data>) {
  return (
    <Workspace
      docKey={data.docKey}
      mode={data.mode}
      isCourse={data.isCourse}
      dirty={data.dirty}
      waiting={data.waiting ?? 0}
      saveStatus={data.saveStatus}
      revision={data.revision}
      outline={data.outline ?? []}
      resume={data.resume}
      savedDoc={data.savedDoc}
      draftDoc={data.draftDoc}
      runs={data.runs ?? []}
      openAt={data.openAt || undefined}
      format={data.format ?? 'json'}
      file={data.file ?? null}
      importNotes={data.importNotes ?? []}
      onShowFile={() => void actions.showFile()}
      onCloseFile={() => actions.closeFile()}
      onCopyFile={as => void actions.copyFile({ as })}
      onDismissNotes={() => actions.dismissNotes()}
      syncDoc={actions.syncDoc}
      onMode={mode => actions.setMode({ mode })}
      onObserve={events => actions.observe({ events })}
      onCopy={text => void actions.copyText({ text })}
      onRun={scenario => actions.run({ scenario })}
      onPick={() => actions.pick()}
      onOpenPage={page => void actions.openPage({ page })}
      onBack={() => void actions.back()}
    />
  );
}
