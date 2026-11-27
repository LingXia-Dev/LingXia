import { cx, useLocal, useView } from '@lingxia/view';
import { Badge } from '@liulidesign/core/Badge';
import { Banner } from '@liulidesign/core/Banner';
import { Button } from '@liulidesign/core/Button';
import { Item, ItemList } from '@liulidesign/core/Item';
import { ListDetail } from '@liulidesign/core/ListDetail';
import type { IconName } from '@liulidesign/core/Icon';
import type { JinshuDocument } from '@liulidesign/jinshu/schema';
import { JinshuView } from '@liulidesign/jinshu/JinshuView';
import type { DocRow } from '../shared/agent';
import { QuizBlockView } from '../shared/QuizBlockView';

const CUSTOM_BLOCKS = { 'x-demo-quiz': QuizBlockView };

const KIND_ICON: Record<string, IconName> = { 课程: 'template', 课程草稿: 'edit', 笔记: 'checklist', 长文: 'rows', 导入: 'download' };
const FORMAT_LABEL = { json: 'JSON', markdown: 'Markdown' } as const;

const percent = (ratio: number) => `${Math.round(ratio * 100)}%`;

function meta(row: DocRow): string {
  return [row.kind, `${row.blocks} 块`, FORMAT_LABEL[row.format], row.autoSave ? '自动保存' : '', row.readRatio !== undefined ? `已读 ${percent(row.readRatio)}` : '']
    .filter(Boolean)
    .join(' · ');
}

/** What waits for the person in a document, most pressing first; empty when nothing does. */
function flags(row: DocRow): { readonly label: string; readonly tone: 'neutral' | 'accent' | 'info' | 'warning' }[] {
  return [
    ...(row.agentChanged ? [{ label: '助手改过', tone: 'accent' as const }] : []),
    ...(row.suggestions > 0 ? [{ label: `${row.suggestions} 条建议`, tone: 'info' as const }] : []),
    ...(row.saveWaiting ? [{ label: '待确认保存', tone: 'info' as const }] : []),
    // An auto-saved document has no unsaved state worth flagging; its description says it saves itself.
    ...(!row.autoSave && row.dirty ? [{ label: '未保存', tone: 'warning' as const }] : []),
  ];
}

/** The most pressing flag, if any. */
function RowEnd({ row }: { readonly row: DocRow }) {
  const first = flags(row)[0];
  return first ? <Badge label={first.label} tone={first.tone} size="sm" /> : null;
}

function Rows({ rows, selected, onChoose }: { readonly rows: readonly DocRow[]; readonly selected?: string; readonly onChoose: (key: string) => void }) {
  return (
    <ItemList label="文档">
      {rows.map(row => {
        return (
          <Item
            key={row.key}
            label={row.title}
            description={meta(row)}
            startIcon={KIND_ICON[row.kind] ?? 'document'}
            endContent={<RowEnd row={row} />}
            isSelected={selected === undefined ? undefined : selected === row.key}
            hasChevron={selected === undefined}
            onClick={() => onChoose(row.key)}
            data-testid={`doc-${row.key}`}
          />
        );
      })}
    </ItemList>
  );
}

function Masthead({ isWide, onImport }: { readonly isWide: boolean; readonly onImport: () => void }) {
  return (
    <view className={cx('gap-1', isWide ? 'px-3 pt-1' : '')}>
      <view className="flex-row items-baseline gap-2">
        <text role="heading" headingLevel={1} className={cx('font-semibold tracking-tight text-text', isWide ? 'text-title' : 'text-display')}>
          锦书
        </text>
        <text className="text-body text-muted">Jinshu</text>
      </view>
      <text className="text-footnote text-muted">人和助手一起编辑的块文档。</text>
      <view className="flex-row pt-2">
        <Button label="从剪贴板导入 Markdown" startIcon="download" size="sm" variant="ghost" edge="start" onClick={onImport} data-testid="import-clipboard" />
      </view>
    </view>
  );
}

function Preview({
  row,
  sections,
  excerpt,
  onOpen,
}: {
  readonly row: DocRow;
  readonly sections: readonly string[];
  readonly excerpt?: JinshuDocument;
  readonly onOpen: (mode?: 'read' | 'edit') => void;
}) {
  const shown = sections.slice(0, 10);
  const waiting = [row.suggestions > 0 ? `${row.suggestions} 条建议` : '', row.saveWaiting ? '一个保存请求' : ''].filter(Boolean).join('，');
  return (
    <view className="gap-8" data-testid="doc-preview">
      <view className="gap-3">
        <text className="text-footnote text-muted">{meta(row)}</text>
        <text role="heading" headingLevel={2} className="text-display font-semibold tracking-tight text-text">{row.title}</text>
        {row.readRatio !== undefined && (
          <view className="h-1 w-56 rounded-full bg-text/8 overflow-hidden">
            <view className="h-full rounded-full bg-accent" style={{ width: `${Math.round(row.readRatio * 100)}%` }} />
          </view>
        )}
        <view className="flex-row gap-2 pt-2">
          <Button label="阅读" variant="primary" onClick={() => onOpen('read')} data-testid="preview-read" />
          <Button label="编辑" onClick={() => onOpen('edit')} data-testid="preview-edit" />
        </view>
      </view>
      {row.agentChanged && <Banner tone="info" title="助手改过这篇" message="你不在时，助手修改了草稿。打开就能看到改了哪里，并逐次撤销。" />}
      {!row.agentChanged && waiting !== '' && <Banner tone="info" title="有事等你审阅" message={waiting} />}
      {/* The contents sit beside the excerpt when the pane is wide, under it when not (a Duo half). */}
      <view className="flex-row flex-wrap items-start gap-x-10 gap-y-8">
        {excerpt && (
          <view className="grow basis-[26rem] min-w-0 rounded-container border border-line bg-surface overflow-hidden">
            <view className="max-h-[440px] overflow-hidden px-7 pt-6">
              <JinshuView doc={excerpt} hasTitle={false} customBlocks={CUSTOM_BLOCKS} />
            </view>
            <view className="flex-row items-center justify-between gap-3 border-t border-line ps-7 pe-3 py-2">
              <text className="text-caption text-muted">{`开头 ${excerpt.blocks.length} 块`}</text>
              <Button label="继续阅读" size="sm" variant="ghost" endIcon="arrow-end" onClick={() => onOpen('read')} />
            </view>
          </view>
        )}
        {shown.length > 0 && (
          <view className="w-52 shrink-0 gap-2.5 pt-1">
            <text role="heading" headingLevel={3} className="text-footnote font-medium text-muted">目录</text>
            <view className="gap-2">
              {shown.map((title, i) => (
                <text key={`${row.key}-s${i}`} className="text-footnote text-text">{title}</text>
              ))}
              {sections.length > shown.length && <text className="text-caption text-muted">{`还有 ${sections.length - shown.length} 节`}</text>}
            </view>
          </view>
        )}
      </view>
    </view>
  );
}

/** The app's documents: a list beside the chosen one's preview on a wide screen, the list alone on a phone. */
export function Library({
  rows,
  sections,
  excerpts,
  onOpen,
  onImport,
}: {
  readonly rows: readonly DocRow[];
  readonly sections: Readonly<Record<string, readonly string[]>>;
  readonly excerpts: Readonly<Record<string, JinshuDocument>>;
  readonly onOpen: (doc: string, mode?: 'read' | 'edit') => void;
  readonly onImport: () => void;
}) {
  const { env } = useView();
  const chosen = useLocal<string | null>('library-chosen', null);

  if (env.sizeClass !== 'regular') {
    return (
      <page className="bg-canvas">
        <scroll className="grow">
          <view className="px-4 pt-2 pb-10 gap-6">
            <Masthead isWide={false} onImport={onImport} />
            <Rows rows={rows} onChoose={key => onOpen(key)} />
            <text className="text-footnote text-faint px-1">在文档页右上角打开「Agent 演练」，看助手怎样通过锦书工具读写文档。</text>
          </view>
        </scroll>
      </page>
    );
  }

  const current = rows.find(row => row.key === chosen.value) ?? rows[0];
  return (
    <page className="bg-canvas">
      <ListDetail value={current?.key ?? null} onChange={next => next !== null && chosen.set(next)} listLabel="文档库" listWidth={340} contentWidth="wide">
        <ListDetail.List>
          <Masthead isWide onImport={onImport} />
          <Rows rows={rows} selected={current?.key} onChoose={key => chosen.set(key)} />
        </ListDetail.List>
        <ListDetail.Detail title={current?.title}>
          {current && <Preview row={current} sections={sections[current.key] ?? []} excerpt={excerpts[current.key]} onOpen={mode => onOpen(current.key, mode)} />}
        </ListDetail.Detail>
      </ListDetail>
    </page>
  );
}
