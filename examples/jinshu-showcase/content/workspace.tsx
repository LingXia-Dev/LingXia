import { cx, useLocal, useView, type Child } from '@lingxia/view';
import { Banner } from '@liulidesign/core/Banner';
import { BottomSheet } from '@liulidesign/core/BottomSheet';
import { Button } from '@liulidesign/core/Button';
import { CodeBlock } from '@liulidesign/core/Code';
import { Dialog } from '@liulidesign/core/Dialog';
import { DropdownMenu } from '@liulidesign/core/DropdownMenu';
import { EmptyState } from '@liulidesign/core/EmptyState';
import { IconButton } from '@liulidesign/core/IconButton';
import { Popover } from '@liulidesign/core/Popover';
import { SegmentedControl } from '@liulidesign/core/SegmentedControl';
import { Split } from '@liulidesign/core/Split';
import { TopBar } from '@liulidesign/core/TopBar';
import type { DiffMode, JinshuDocument, JinshuProgress, Observation, OutlineItem } from '@liulidesign/jinshu/schema';
import { JinshuDiff } from '@liulidesign/jinshu/JinshuDiff';
import { JinshuEditor } from '@liulidesign/jinshu/JinshuEditor';
import { JinshuOutline, useJinshuNav } from '@liulidesign/jinshu/JinshuOutline';
import { JinshuView, type JinshuChannelAction } from '@liulidesign/jinshu/JinshuView';
import type { RunLog, ScenarioId, ShownFile } from '../shared/agent';
import { QuizBlockView } from '../shared/QuizBlockView';
import { AgentDrill, RunStrip } from './agentDrill';

export type DocMode = 'read' | 'edit' | 'changes';

export interface WorkspaceProps {
  readonly docKey: string;
  readonly mode: DocMode;
  readonly isCourse: boolean;
  readonly dirty: boolean;
  readonly waiting: number;
  readonly saveStatus: 'saved' | 'dirty' | 'saving' | 'failed';
  readonly revision: number;
  readonly outline: readonly OutlineItem[];
  readonly resume?: JinshuProgress;
  readonly savedDoc: JinshuDocument | null;
  readonly draftDoc: JinshuDocument | null;
  readonly runs: readonly RunLog[];
  readonly openAt?: string;
  /** How the document is stored. */
  readonly format: 'json' | 'markdown';
  /** The file as `save` would write it, while the dialog shows it. */
  readonly file: ShownFile | null;
  /** What an imported Markdown text could not keep; shown until dismissed. */
  readonly importNotes: readonly string[];
  readonly onShowFile: () => void;
  readonly onCloseFile: () => void;
  /** Copies the whole file, or its Markdown export, from Logic. */
  readonly onCopyFile: (as: 'file' | 'markdown') => void;
  readonly onDismissNotes: () => void;
  readonly syncDoc: JinshuChannelAction;
  readonly onMode: (mode: DocMode) => void;
  readonly onObserve: (events: readonly Observation[]) => void;
  readonly onCopy: (text: string) => void;
  readonly onRun: (scenario: ScenarioId) => void;
  /** Phone: the system sheet of scenarios. */
  readonly onPick: () => void;
  readonly onOpenPage: (page: string) => void;
  readonly onBack: () => void;
}

const CUSTOM_BLOCKS = { 'x-demo-quiz': QuizBlockView };
const FILE_POLICY = { default: 'attachment', inline: 'never' } as const;
const COURSE_MEDIA = { seek: 'played-only', maxRate: 1.5 } as const;

/** One document, read, edited and compared in one place; the agent drill opens beside it on request. */
export function Workspace(props: WorkspaceProps) {
  const { env } = useView();
  const isWide = env.sizeClass === 'regular';
  const assistantOpen = useLocal('assistant-open', false);
  const diffMode = useLocal<DiffMode>('diff-mode', 'smart');
  const dismissed = useLocal('run-dismissed', '');
  const latest = props.runs[props.runs.length - 1];
  const nav = useJinshuNav(`nav-${props.docKey}`);
  const outlineOpen = useLocal('outline-open', false);

  const toolbar = (
    <TopBar
      startContent={<IconButton label="文档库" icon="chevron-start" onClick={props.onBack} data-testid="doc-back" />}
      centerContent={
        <SegmentedControl
          label="文档模式"
          size="sm"
          value={props.mode}
          onChange={v => props.onMode(v as DocMode)}
          items={[
            { value: 'read', label: '阅读' },
            { value: 'edit', label: '编辑' },
            { value: 'changes', label: '改动', isDisabled: !props.dirty && props.mode !== 'changes' },
          ]}
          data-testid="doc-mode"
        />
      }
      endContent={
        // A phone keeps the bar one row: the actions go behind one More, mirroring the back button.
        !isWide ? (
          <DropdownMenu
            label="更多"
            alignment="end"
            items={[
              ...(props.mode === 'read' && props.outline.length > 0
                ? [{ label: '目录', icon: 'list' as const, onClick: () => outlineOpen.set(true), 'data-testid': 'doc-outline' }]
                : []),
              ...(props.docKey !== '' ? [{ label: '源文件', icon: 'file' as const, onClick: props.onShowFile, 'data-testid': 'doc-file' }] : []),
              { label: 'Agent 演练', icon: 'play' as const, onClick: props.onPick, 'data-testid': 'assistant-open' },
            ]}
          >
            <IconButton label="更多" icon="more-vertical" data-testid="doc-more" />
          </DropdownMenu>
        ) : (
        <view className="flex-row items-center gap-1">
          {props.mode === 'read' && props.outline.length > 0 && (
            <Popover label="目录" title="目录" alignment="end" content={<JinshuOutline items={props.outline} nav={nav} isTitleHidden className="w-64" />}>
              <IconButton label="目录" icon="list" data-testid="doc-outline" />
            </Popover>
          )}
          {props.docKey !== '' && <IconButton label="源文件" icon="file" onClick={props.onShowFile} data-testid="doc-file" />}
          <IconButton
            label={isWide && assistantOpen.value ? '收起 Agent 演练' : 'Agent 演练'}
            icon="play"
            variant={isWide && assistantOpen.value ? 'tonal' : 'ghost'}
            onClick={() => (isWide ? assistantOpen.set(!assistantOpen.value) : props.onPick())}
            data-testid="assistant-open"
          />
        </view>
        )
      }
      className="border-b border-line"
    />
  );

  const column = (child: Child, onVisibleChange?: typeof nav.onVisibleChange) => (
    <scroll onVisibleChange={onVisibleChange} className="grow basis-0 min-h-0">
      <view className={cx('w-full max-w-[74ch] self-center', isWide ? 'px-8 pt-6 pb-24' : 'px-4 pt-4 pb-24')}>{child}</view>
    </scroll>
  );

  const content =
    props.docKey === '' ? (
      <view className="grow" />
    ) : props.mode === 'edit' ? (
      <scroll className="grow basis-0 min-h-0">
        <view className={cx('w-full', isWide ? 'px-8 pb-24' : 'px-4 pb-24')}>
          <JinshuEditor
            channel={props.syncDoc}
            docKey={props.docKey}
            isDirty={props.dirty}
            saveStatus={props.saveStatus}
            isToolbarSticky
            customBlocks={CUSTOM_BLOCKS}
            openAt={props.openAt}
            hasLeaveGuard
          />
        </view>
      </scroll>
    ) : props.mode === 'changes' ? (
      column(
        props.dirty && props.savedDoc && props.draftDoc ? (
          <view className="gap-5">
            <view className="flex-row flex-wrap items-center justify-between gap-3">
              <text className="text-footnote text-muted">草稿与上次保存的版本相比</text>
              <SegmentedControl
                label="对比粒度"
                size="sm"
                value={diffMode.value}
                onChange={v => diffMode.set(v as DiffMode)}
                items={[
                  { value: 'smart', label: '智能' },
                  { value: 'inline', label: '逐词' },
                  { value: 'block', label: '按块' },
                ]}
              />
            </view>
            <JinshuDiff a={props.savedDoc} b={props.draftDoc} mode={diffMode.value} />
          </view>
        ) : (
          <EmptyState title="没有未保存的改动" description="草稿和上次保存的版本一致。编辑或让助手修改后，改动会显示在这里。" className="pt-16" />
        ),
      )
    ) : (
      column(
        <view className="gap-6">
          {/* Review happens in the editor; reading still says what waits, one tap away. */}
          {props.waiting > 0 && (
            <Banner
              tone="info"
              title={`${props.waiting} 项等你审阅`}
              message="助手的建议或保存请求在编辑模式里逐条处理。"
              action={{ label: '去审阅', onClick: () => props.onMode('edit') }}
              data-testid="waiting-banner"
            />
          )}
          <JinshuView
          channel={props.syncDoc}
          docKey={props.docKey}
          nav={nav}
          filePolicy={FILE_POLICY}
          mediaPolicy={props.isCourse ? COURSE_MEDIA : undefined}
          customBlocks={CUSTOM_BLOCKS}
          hasUpdateHighlights
          resume={props.resume}
          openAt={props.openAt}
          onCopyCode={code => props.onCopy(code)}
          onObserve={props.onObserve}
          />
        </view>,
        nav.onVisibleChange,
      )
    );

  const notes =
    props.importNotes.length > 0 ? (
      <view className={cx('w-full max-w-[74ch] self-center', isWide ? 'px-8 pt-4' : 'px-4 pt-3')}>
        <Banner
          tone="warning"
          title="导入时有些内容无法原样保留"
          message={props.importNotes.join('、')}
          onDismiss={props.onDismissNotes}
          data-testid="import-notes"
        />
      </view>
    ) : null;

  const panel = (
    <AgentDrill runs={props.runs} onRun={scenario => {
        // Phone: the sheet steps aside so the change lands in view.
        if (!isWide) assistantOpen.set(false);
        props.onRun(scenario);
      }} onOpenPage={props.onOpenPage} onClose={isWide ? undefined : () => assistantOpen.set(false)} />
  );

  // The drill stands in for an agent outside the app. Wide: beside the document, not modal, so changes
  // land in view. Phone: a system sheet picks a scenario and closes, a strip reports it, and the full
  // log opens only on request, so the document is never covered while it changes.
  return (
    <page className="bg-canvas">
      {toolbar}
      {notes}
      {/* Keyed by document: its tab and its file never carry over to another one. */}
      <FileDialog key={`file-${props.docKey}`} docKey={props.docKey} file={props.file?.doc === props.docKey ? props.file : null} onClose={props.onCloseFile} onCopy={props.onCopyFile} />
      {isWide ? (
        assistantOpen.value ? (
          <Split asideLabel="Agent 演练" asideWidth={340} aside={panel} data-testid="doc-split">
            {content}
          </Split>
        ) : (
          content
        )
      ) : (
        <>
          {content}
          {latest && dismissed.value !== latest.id && (
            <RunStrip
              run={latest}
              onDetails={() => assistantOpen.set(true)}
              onOpenPage={props.onOpenPage}
              onDismiss={() => dismissed.set(latest.id)}
            />
          )}
          <BottomSheet isOpen={assistantOpen.value} onOpenChange={open => assistantOpen.set(open)} label="Agent 演练" isLabelHidden className="px-0 pb-2 gap-0">
            <view style={{ height: Math.round(env.viewport.height * 0.7) }}>{panel}</view>
          </BottomSheet>
          <BottomSheet isOpen={outlineOpen.value} onOpenChange={open => outlineOpen.set(open)} label="目录">
            <JinshuOutline items={props.outline} nav={nav} isTitleHidden onSelect={() => outlineOpen.set(false)} />
          </BottomSheet>
        </>
      )}
    </page>
  );
}

/** The document's file exactly as saving would write it; a JSON document can also be read as Markdown. */
function FileDialog({
  docKey,
  file,
  onClose,
  onCopy,
}: {
  readonly docKey: string;
  readonly file: ShownFile | null;
  readonly onClose: () => void;
  readonly onCopy: (as: 'file' | 'markdown') => void;
}) {
  const shown = useLocal<'file' | 'markdown'>(`file-shown-${docKey}`, 'file');
  const asMarkdown = file?.format === 'json' && shown.value === 'markdown';
  const text = !file ? '' : asMarkdown && file.format === 'json' ? file.markdown : file.text;
  const more = !file ? 0 : asMarkdown ? (file.markdownMore ?? 0) : file.more;
  const name = file?.format === 'markdown' || asMarkdown ? 'document.md' : 'document.json';
  return (
    <Dialog
      isOpen={file !== null}
      onOpenChange={open => {
        if (!open) {
          shown.set('file');
          onClose();
        }
      }}
      label="源文件"
      description={file?.format === 'markdown' ? '以 Markdown 保存，未改动的部分保持原样。' : asMarkdown ? '把这篇导出为 Markdown 的样子；文档仍以 JSON 保存。' : '保存时写入的 JSON。'}
      size="lg"
      data-testid="file-dialog"
    >
      {file && (
        <view className="gap-3">
          {file.format === 'json' && (
            <SegmentedControl
              label="查看"
              size="sm"
              value={shown.value}
              onChange={v => shown.set(v as 'file' | 'markdown')}
              items={[
                { value: 'file', label: 'JSON' },
                { value: 'markdown', label: '导出为 Markdown' },
              ]}
              data-testid="file-shown"
            />
          )}
          {asMarkdown && file.format === 'json' && file.lost.length > 0 && (
            <Banner tone="warning" title="Markdown 无法保留这些内容" message={file.lost.join('、')} data-testid="export-lost" />
          )}
          {/* The dialog's body scrolls a long file. */}
          <CodeBlock
            code={text}
            label={name}
            title={name}
            language={name.endsWith('.json') ? 'json' : undefined}
            action={<Button label="复制" startIcon="copy" size="sm" variant="ghost" onClick={() => onCopy(asMarkdown ? 'markdown' : 'file')} data-testid="file-copy" />}
            data-testid="file-text"
          />
          {more > 0 && <text className="text-caption text-muted">{`还有 ${more} 行未显示，复制可得全文。`}</text>}
        </view>
      )}
    </Dialog>
  );
}
