import { t, useAction, useLocal, useView, type MessageKey } from '@lingxia/view';
import { Banner } from '@liulidesign/core/Banner';
import { Button } from '@liulidesign/core/Button';
import { EmptyState } from '@liulidesign/core/EmptyState';
import { Icon } from '@liulidesign/core/Icon';
import { IconButton } from '@liulidesign/core/IconButton';
import { ItemList } from '@liulidesign/core/Item';
import { Link } from '@liulidesign/core/Link';
import { ProgressBar } from '@liulidesign/core/ProgressBar';
import { SearchField } from '@liulidesign/core/SearchField';
import { SegmentedControl } from '@liulidesign/core/SegmentedControl';
import { formatTimestamp } from '@liulidesign/core/Timestamp';
import { formatBytes, hostOf } from '../../shared/format';
import type { View } from './$types';

type Data = View['data'];
type Download = Data['downloads'][number];

const FILTERS = ['all', 'active', 'completed'] as const;

const isActive = (item: Download) => item.status === 'downloading' || item.status === 'paused';

/** Downloading and Paused first, then finished downloads by the day they finished. */
function groupOf(item: Download, data: Data, isTwelveHour: boolean): string {
  if (isActive(item)) return t(`downloads.status.${item.status}` as MessageKey);
  const day = `${item.at.slice(0, 10)}T00:00`;
  if (day === data.today) return t('history.today');
  if (day === data.yesterday) return t('history.yesterday');
  return formatTimestamp(day, data.now, 'date', isTwelveHour);
}

export function render({ data, actions }: View) {
  const query = useLocal('query', '');
  const filter = useLocal<string>('filter', 'all');
  const pause = useAction(actions.pause);
  const resume = useAction(actions.resume);
  const retry = useAction(actions.retry);
  const cancel = useAction(actions.cancel);
  const remove = useAction(actions.remove);
  const open = useAction(actions.open);
  const reveal = useAction(actions.reveal);
  const clearCompleted = useAction(actions.clearCompleted);
  const openSettings = useAction(actions.openSettings);
  const reload = useAction(actions.reload);
  const isTwelveHour = useView().env.hourCycle === 'h12';

  const q = query.value.trim().toLowerCase();
  const visible = data.downloads.filter((item) => {
    if (filter.value === 'active' && !isActive(item)) return false;
    if (filter.value === 'completed' && item.status !== 'completed') return false;
    return !q || item.fileName.toLowerCase().includes(q) || item.url.toLowerCase().includes(q);
  });
  const ordered = [...visible.filter(isActive).sort((a, b) => (a.status === b.status ? 0 : a.status === 'downloading' ? -1 : 1)), ...visible.filter((item) => !isActive(item))];
  const groups: { label: string; items: Download[] }[] = [];
  for (const item of ordered) {
    const label = groupOf(item, data, isTwelveHour);
    const last = groups[groups.length - 1];
    if (last && last.label === label) last.items.push(item);
    else groups.push({ label, items: [item] });
  }
  const activeCount = data.downloads.filter(isActive).length;
  const fileCount = data.downloads.length - activeCount;

  const sizeText = (item: Download) => (item.totalBytes
    ? t('downloads.size.of', { done: formatBytes(item.downloadedBytes), total: formatBytes(item.totalBytes) })
    : formatBytes(item.downloadedBytes));

  const detail = (item: Download) => {
    if (item.status === 'failed') {
      return <text className="text-footnote text-danger break-words">{item.failure ?? t('downloads.status.failed')}</text>;
    }
    const parts = item.status === 'completed'
      ? [formatBytes(item.totalBytes ?? item.downloadedBytes), formatTimestamp(item.at, data.now, 'date-time', isTwelveHour), hostOf(item.url)]
      : item.status === 'downloading'
        ? [sizeText(item), item.speed > 0 ? `${formatBytes(item.speed)}/s` : '', hostOf(item.url)]
        : item.status === 'paused'
          ? [sizeText(item), t('downloads.status.paused')]
          : [t(`downloads.status.${item.status}` as MessageKey)];
    const line = <text className="text-footnote text-muted truncate">{parts.filter(Boolean).join(' · ')}</text>;
    if (!isActive(item)) return line;
    return (
      <view className="gap-1 pt-1">
        <ProgressBar
          label={item.fileName}
          isLabelHidden
          value={item.totalBytes ? item.downloadedBytes : undefined}
          max={item.totalBytes ?? undefined}
          valueLabel={sizeText(item)}
          size="sm"
        />
        {line}
      </view>
    );
  };

  const controls = (item: Download) => (
    <view className="shrink-0 flex-row items-center gap-1 text-muted">
      {item.status === 'downloading' ? (
        <IconButton label={t('downloads.pause', { name: item.fileName })} icon="pause" size="sm" clickAction={() => pause.run({ taskId: item.taskId })} />
      ) : null}
      {item.status === 'paused' ? (
        <IconButton label={t('downloads.resume', { name: item.fileName })} icon="play" size="sm" clickAction={() => resume.run({ taskId: item.taskId })} />
      ) : null}
      {isActive(item) ? (
        <IconButton label={t('downloads.cancel', { name: item.fileName })} icon="cancel" size="sm" clickAction={() => cancel.run({ taskId: item.taskId })} />
      ) : null}
      {item.status === 'failed' ? (
        <Button label={t('downloads.retry')} size="sm" variant="secondary" clickAction={() => retry.run({ taskId: item.taskId })} />
      ) : null}
      {item.status === 'completed' ? (
        <IconButton label={t('downloads.reveal', { name: item.fileName })} icon="folder" size="sm" clickAction={() => reveal.run({ taskId: item.taskId })} />
      ) : null}
      {isActive(item) ? null : (
        <IconButton
          label={t('downloads.remove', { name: item.fileName })}
          icon="close"
          size="sm"
          edge="end"
          clickAction={() => remove.run({ taskId: item.taskId })}
          data-testid={`download-remove-${item.taskId}`}
        />
      )}
    </view>
  );

  // Item's label cannot be a link, and a finished file opens from its name, so the row is drawn here in Item's shape.
  const row = (item: Download) => (
    <view key={item.taskId} className="flex-row items-center gap-3 px-4 py-2.5 min-h-control-lg self-stretch" data-testid={`download-${item.taskId}`}>
      <view className={`size-8 shrink-0 items-center justify-center rounded-control ${item.status === 'failed' ? 'bg-danger/12 text-danger' : 'bg-accent-soft text-on-accent-soft'}`}>
        <Icon name={item.status === 'failed' ? 'error' : isActive(item) ? 'download' : 'file'} />
      </view>
      <view className="grow shrink basis-0 min-w-0 gap-0.5">
        {item.status === 'completed'
          ? <view className="flex-row min-w-0"><Link label={item.fileName} onClick={() => { void open.run({ taskId: item.taskId }); }} data-testid={`download-open-${item.taskId}`} /></view>
          : <text className="text-body text-text truncate">{item.fileName}</text>}
        {detail(item)}
      </view>
      {controls(item)}
    </view>
  );

  return (
    <page data-testid="downloads-page" className="bg-canvas text-text">
      <scroll className="grow bg-canvas">
        <view className="w-full max-w-[880px] self-center px-6 py-4 gap-4">
          <view className="flex-row items-center gap-3">
            <SegmentedControl
              label={t('downloads.filter')}
              bind={filter}
              items={FILTERS.map((value) => ({ value, label: t(`downloads.filter.${value}` as MessageKey) }))}
              size="sm"
              className="w-72 shrink-0"
              data-testid="downloads-filter"
            />
            <SearchField
              label={t('downloads.search')}
              isLabelHidden
              placeholder={t('downloads.search')}
              bind={query}
              size="sm"
              className="grow basis-0 min-w-0"
            />
            <Button
              label={t('downloads.clearCompleted')}
              variant="secondary"
              size="sm"
              isDisabled={!data.downloads.some((item) => !isActive(item))}
              clickAction={() => clearCompleted.run()}
            />
            <IconButton label={t('downloads.settings')} icon="settings" size="sm" edge="end" clickAction={() => openSettings.run()} data-testid="downloads-settings-btn" />
          </view>
          {data.loadFailed ? (
            <Banner
              title={t('downloads.loadFailed')}
              tone="danger"
              action={{ label: t('downloads.retry'), onClick: () => { void reload.run(); } }}
              data-testid="downloads-load-failed"
            />
          ) : null}
          {data.downloads.length ? (
            <text className="text-footnote text-muted" data-testid="downloads-summary">
              {[activeCount ? t('downloads.summary.active', { count: activeCount }) : '', t('downloads.summary.files', { count: fileCount })].filter(Boolean).join(' · ')}
            </text>
          ) : null}
          {data.loading || data.loadFailed ? null : visible.length === 0 ? (
            <EmptyState
              title={data.downloads.length ? t('downloads.noMatches') : t('downloads.empty')}
              description={data.downloads.length ? undefined : t('downloads.emptyHint')}
              size="sm"
              data-testid="downloads-empty"
            />
          ) : (
            groups.map((group) => (
              <ItemList key={group.label} title={group.label}>
                {group.items.map(row)}
              </ItemList>
            ))
          )}
        </view>
      </scroll>
    </page>
  );
}
