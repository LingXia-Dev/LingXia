import { cx, useLocal } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { IconButton } from '@liulidesign/core/IconButton';
import { Item, ItemList } from '@liulidesign/core/Item';
import { StatusDot } from '@liulidesign/core/StatusDot';
import { SCENARIOS, type CallLog, type RunLog, type ScenarioId } from '../shared/agent';

export interface AgentDrillProps {
  readonly runs: readonly RunLog[];
  readonly onRun: (scenario: ScenarioId) => void;
  readonly onOpenPage: (page: string) => void;
  readonly onClose?: () => void;
}

const CALL_TONE = { ok: 'success', 'pending-review': 'info', error: 'danger', running: 'neutral' } as const;
const CALL_WORD = { ok: '成功', 'pending-review': '等你审阅', error: '失败', running: '进行中' } as const;
const titleOf = (id: ScenarioId) => SCENARIOS.find(s => s.id === id)?.title ?? id;

function Call({ call, isOpen, onToggle }: { readonly call: CallLog; readonly isOpen: boolean; readonly onToggle: () => void }) {
  return (
    <view className="gap-1.5">
      <view className="flex-row items-center gap-2">
        <StatusDot tone={CALL_TONE[call.status]} label={CALL_WORD[call.status]} size="sm" />
        <text className="shrink-0 text-caption font-mono font-medium text-text">{call.name}</text>
        <text className="grow min-w-0 text-caption font-mono text-muted truncate">{call.args}</text>
        {call.result && <IconButton label={isOpen ? '收起结果' : '展开结果'} icon={isOpen ? 'chevron-up' : 'chevron-down'} size="sm" onClick={onToggle} />}
      </view>
      {isOpen && call.result && (
        <scroll className="max-h-40 rounded-control bg-surface-sunken">
          <text className="p-2.5 text-caption font-mono leading-relaxed text-text whitespace-pre-wrap select-text">{call.result}</text>
        </scroll>
      )}
    </view>
  );
}

/** The latest run: what was called, what came back, what it means. */
function LatestRun({ run, onOpenPage }: { readonly run: RunLog; readonly onOpenPage: AgentDrillProps['onOpenPage'] }) {
  // Results fold so the scenarios stay in view; a read's markup is the point, so it shows open.
  const open = useLocal<string>('drill-open', '');
  const isOpen = (i: number) =>
    open.value.startsWith(`${run.id}:`) ? open.value === `${run.id}:${i}` : run.scenario === 'read' && i === run.calls.length - 1;
  return (
    <view className="gap-2.5" data-testid="drill-latest">
      <view className="flex-row items-center gap-2">
        <StatusDot tone={run.status === 'running' ? 'neutral' : run.status === 'done' ? 'success' : 'warning'} label={run.status === 'running' ? '进行中' : run.status === 'done' ? '完成' : '没有完成'} size="sm" />
        <text className="text-footnote font-semibold text-text">{titleOf(run.scenario)}</text>
      </view>
      {run.calls.map((call, i) => (
        <Call key={`${run.id}-${i}`} call={call} isOpen={isOpen(i)} onToggle={() => open.set(isOpen(i) ? `${run.id}:none` : `${run.id}:${i}`)} />
      ))}
      {run.outcome && <text className={cx('text-footnote', run.status === 'failed' ? 'text-warning' : 'text-text')}>{run.outcome}</text>}
      {run.page && (
        <view className="flex-row">
          <Button label={`打开《${run.pageTitle ?? ''}》`} size="sm" variant="tonal" endIcon="arrow-end" onClick={() => onOpenPage(run.page!)} />
        </view>
      )}
    </view>
  );
}

/**
 * Drives the document the way an outside agent would. Not part of Jinshu, which has no AI surface, nor of
 * a real app: in a host the agent lives elsewhere and reaches the tools the app registered at launch.
 */
export function AgentDrill({ runs, onRun, onOpenPage, onClose }: AgentDrillProps) {
  const latest = runs[runs.length - 1];
  const running = new Set(runs.filter(r => r.status === 'running').map(r => r.scenario));
  const rows = (via: 'tools' | 'app') =>
    SCENARIOS.filter(s => s.via === via).map(s => (
      <Item
        key={s.id}
        label={s.title}
        description={s.watch}
        startIcon="play"
        isDisabled={running.has(s.id)}
        endContent={running.has(s.id) ? <StatusDot tone="neutral" label="进行中" size="sm" /> : undefined}
        onClick={() => onRun(s.id)}
        data-testid={`drill-${s.id}`}
      />
    ));
  return (
    <view className="grow basis-0 min-h-0">
      <view className="shrink-0 flex-row items-start gap-2 px-4 pt-3 pb-2">
        <view className="grow min-w-0 gap-0.5">
          <text role="heading" headingLevel={2} className="text-callout font-semibold text-text">Agent 演练</text>
          <text className="text-caption text-muted">锦书没有 AI 界面。宿主里的 Agent 只通过应用注册的工具读写文档；这里替它发起调用。</text>
        </view>
        {onClose && <IconButton label="关闭" icon="close" size="sm" onClick={onClose} />}
      </view>
      <scroll className="grow basis-0 min-h-0">
        <view className="gap-5 px-3 pt-1 pb-4">
          <ItemList title="Agent 通过工具" density="compact">{rows('tools')}</ItemList>
          <ItemList title="应用自己的代码" density="compact">{rows('app')}</ItemList>
        </view>
      </scroll>
      {latest && (
        <scroll className="shrink-0 max-h-[40%] border-t border-line">
          <view className="px-4 pt-3 pb-4">
            <LatestRun run={latest} onOpenPage={onOpenPage} />
          </view>
        </scroll>
      )}
    </view>
  );
}

/** Phone: the latest run in one line under the document, with the log one tap away. */
export function RunStrip({
  run,
  onDetails,
  onOpenPage,
  onDismiss,
}: {
  readonly run: RunLog;
  readonly onDetails: () => void;
  readonly onOpenPage: AgentDrillProps['onOpenPage'];
  readonly onDismiss: () => void;
}) {
  const word = run.status === 'running' ? '进行中' : run.status === 'done' ? '完成' : '没有完成';
  return (
    <view className="shrink-0 gap-2 border-t border-line bg-surface ps-4 pe-2 pt-2.5 pb-3" role="status" data-testid="run-strip">
      <view className="flex-row items-center gap-2">
        <StatusDot tone={run.status === 'running' ? 'neutral' : run.status === 'done' ? 'success' : 'warning'} label={word} size="sm" />
        <text className="grow min-w-0 text-footnote font-medium text-text truncate">{`Agent · ${titleOf(run.scenario)}`}</text>
        <Button label="详情" size="sm" variant="ghost" onClick={onDetails} data-testid="run-details" />
        <IconButton label="关闭" icon="close" size="sm" onClick={onDismiss} />
      </view>
      {run.outcome && <text className={cx('text-footnote pe-2', run.status === 'failed' ? 'text-warning' : 'text-muted')}>{run.outcome}</text>}
      {run.page && (
        <view className="flex-row">
          <Button label={`打开《${run.pageTitle ?? ''}》`} size="sm" variant="tonal" endIcon="arrow-end" onClick={() => onOpenPage(run.page!)} />
        </view>
      )}
    </view>
  );
}
