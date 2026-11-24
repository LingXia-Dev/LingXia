import { List, t, useAction, useLocal, useView } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { EmptyState } from '@liulidesign/core/EmptyState';
import { IconButton } from '@liulidesign/core/IconButton';
import { SearchField } from '@liulidesign/core/SearchField';
import { formatTimestamp } from '@liulidesign/core/Timestamp';
import { ToastRegion, useToast } from '@liulidesign/core/Toast';
import { TopBar } from '@liulidesign/core/TopBar';
import { ClearDataDialog } from '../../shared/ClearDataDialog';
import { RowMenu, type MenuAsk } from '../../shared/RowMenu';
import { SiteIcon } from '../../shared/SiteIcon';
import type { View } from './$types';

// Fixed heights let the list place any of thousands of rows without measuring them.
const DAY_HEIGHT = 48;
const VISIT_HEIGHT = 44;

type Data = View['data'];
type Row = Data['rows'][number];

export function render(view: View) {
  return (
    <page data-testid="history-page" className="bg-surface text-text">
      <ToastRegion>
        <History {...view} />
      </ToastRegion>
    </page>
  );
}

type MenuTarget = { id: string; url: string; title: string };

function dayLabel(day: string, data: Data, isTwelveHour: boolean): string {
  if (day === data.today) return t('history.today');
  if (day === data.yesterday) return t('history.yesterday');
  return formatTimestamp(day, data.now, 'date', isTwelveHour);
}

function DayRow({ label }: { label: string }) {
  return (
    <view className="h-12 justify-end px-4 pb-2">
      <text role="heading" headingLevel={2} className="text-footnote font-semibold text-muted">{label}</text>
    </view>
  );
}

/**
 * One visit. Props are plain values, so a row whose visit did not change is not rendered again; the
 * remove button mounts only under a hovering pointer, and the row's menu covers touch and keys.
 */
function VisitRow({ id, url, title, host, at, isTwelveHour, onOpen, onRemove, onMenu }: {
  id: string;
  url: string;
  title: string;
  host: string;
  at: string;
  isTwelveHour: boolean;
  onOpen: (url: string) => void;
  onRemove: (id: string) => void;
  onMenu: (target: MenuTarget, at: MenuAsk) => void;
}) {
  const hovered = useLocal('hovered', false);
  return (
    <view
      onPointerEnter={() => hovered.set(true)}
      onPointerLeave={() => hovered.set(false)}
      className="h-11 flex-row items-center rounded-control hover:bg-text/4"
      data-testid={`history-row-${id}`}
    >
      <button
        aria-label={title}
        onClick={() => onOpen(url)}
        onContextMenu={(event) => onMenu({ id, url, title }, event)}
        className="grow shrink basis-0 min-w-0 self-stretch flex-row items-center gap-3 px-4 rounded-control bg-transparent text-start active:bg-text/7 focus-visible:inset-ring-2 focus-visible:inset-ring-focus"
      >
        <text className="w-20 shrink-0 text-footnote text-muted tabular-nums">{formatTimestamp(at, at, 'time', isTwelveHour)}</text>
        <SiteIcon host={host} size="sm" />
        <text className="shrink min-w-0 truncate text-body text-text">{title}</text>
        <text className="shrink-[4] min-w-0 truncate text-footnote text-muted">{host}</text>
      </button>
      {hovered.value ? (
        <IconButton
          label={t('history.remove', { title })}
          icon="close"
          size="sm"
          className="me-2 text-muted"
          onClick={() => onRemove(id)}
          data-testid={`history-remove-${id}`}
        />
      ) : null}
    </view>
  );
}

function History({ data, actions }: View) {
  const query = useLocal('query', '');
  const clearing = useLocal('clearing', false);
  const menu = useLocal<{ target: MenuTarget; at: MenuAsk } | null>('menu', null);
  const search = useAction(actions.search);
  const clearBrowsingData = useAction(actions.clearBrowsingData);
  const loadMore = useAction(actions.loadMore);
  const toast = useToast();
  const isTwelveHour = useView().env.hourCycle === 'h12';

  // Rows call the actions themselves: a closure over them keeps its identity, so rows stay unrendered.
  const open = (url: string) => { void actions.openUrl({ url }); };
  const remove = (id: string) => { void actions.removeEntry({ id }); };
  const ask = (target: MenuTarget, at: MenuAsk) => menu.set({ target, at });
  const renderRow = (row: Row) => (row.kind === 'day'
    ? <DayRow label={dayLabel(row.day, data, isTwelveHour)} />
    : <VisitRow id={row.id} url={row.url} title={row.title || row.url} host={row.host} at={row.at} isTwelveHour={isTwelveHour} onOpen={open} onRemove={remove} onMenu={ask} />);
  const target = menu.value?.target;

  return (
    <view className="grow min-h-0">
      <TopBar
        heading={t('history.title')}
        endContent={
          <Button
            label={t('history.clearData')}
            variant="secondary"
            size="sm"
            onClick={() => clearing.set(true)}
            data-testid="history-clear-data"
          />
        }
        className="px-6"
      />
      <view className="grow min-h-0 bg-canvas">
      <view className="grow min-h-0 w-full max-w-[880px] self-center px-4 pt-4 gap-2">
        <view className="px-2">
          <SearchField
            label={t('history.search')}
            isLabelHidden
            placeholder={t('history.search')}
            value={query.value}
            onChange={(value) => { query.set(value); void search.run({ query: value }); }}
            resultsLabel={query.value.trim() ? t('history.results', { count: data.shown }) : undefined}
            isResultsLabelHidden
            data-testid="history-search"
          />
        </view>
        {data.loading ? null : data.rows.length === 0 ? (
          <EmptyState
            title={query.value.trim() ? t('history.noMatches') : t('history.empty')}
            description={query.value.trim() ? undefined : t('history.emptyHint')}
            className="grow justify-center"
            data-testid="history-empty"
          />
        ) : (
          <List
            items={data.rows}
            getKey={(row) => row.key}
            getItemHeight={(row) => (row.kind === 'day' ? DAY_HEIGHT : VISIT_HEIGHT)}
            renderItem={renderRow}
            onReachEnd={() => { if (data.hasMore) void loadMore.run(); }}
            className="grow pb-6"
            data-testid="history-list"
          />
        )}
      </view>
      </view>
      <RowMenu
        label={target ? target.title : t('history.title')}
        at={menu.value?.at ?? null}
        items={target ? [
          { label: t('history.open'), icon: 'external-link', onClick: () => open(target.url) },
          { type: 'divider' },
          { label: t('history.removeShort'), icon: 'delete', variant: 'destructive', onClick: () => remove(target.id) },
        ] : []}
        onClose={() => menu.set(null)}
        data-testid="history-row-menu"
      />
      <ClearDataDialog
        isOpen={clearing.value}
        onOpenChange={(next) => clearing.set(next)}
        onClear={async (input) => {
          const outcome = await clearBrowsingData.run(input);
          if (outcome.ok) toast.show({ message: t('privacy.cleared', { count: outcome.value.historyRemoved }), tone: 'success' });
        }}
      />
    </view>
  );
}
