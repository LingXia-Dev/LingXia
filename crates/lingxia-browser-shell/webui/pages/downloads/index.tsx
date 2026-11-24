import { t, useAction, useLocal, type MessageKey } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { EmptyState } from '@liulidesign/core/EmptyState';
import { IconButton } from '@liulidesign/core/IconButton';
import { Item, ItemList } from '@liulidesign/core/Item';
import { ProgressBar } from '@liulidesign/core/ProgressBar';
import { SearchField } from '@liulidesign/core/SearchField';
import { SegmentedControl } from '@liulidesign/core/SegmentedControl';
import { TopBar } from '@liulidesign/core/TopBar';
import { formatBytes } from '../../shared/format';
import type { View } from './$types';

type Download = View['data']['downloads'][number];

const FILTERS = ['all', 'active', 'completed'] as const;

const isActive = (item: Download) => item.status === 'downloading' || item.status === 'paused';

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

  const q = query.value.trim().toLowerCase();
  const visible = data.downloads.filter((item) => {
    if (filter.value === 'active' && !isActive(item)) return false;
    if (filter.value === 'completed' && item.status !== 'completed') return false;
    return !q || item.fileName.toLowerCase().includes(q) || item.url.toLowerCase().includes(q);
  });

  const sizeText = (item: Download) => (item.totalBytes
    ? t('downloads.size.of', { done: formatBytes(item.downloadedBytes), total: formatBytes(item.totalBytes) })
    : formatBytes(item.downloadedBytes));

  const description = (item: Download) => {
    const status = t(`downloads.status.${item.status}` as MessageKey);
    if (!isActive(item)) {
      const size = item.status === 'completed' ? formatBytes(item.totalBytes ?? item.downloadedBytes) : '';
      return size ? `${size} · ${status}` : status;
    }
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
        <text className="text-footnote text-muted">{`${sizeText(item)} · ${status}`}</text>
      </view>
    );
  };

  const controls = (item: Download) => (
    <view className="flex-row items-center gap-1">
      {item.status === 'downloading' ? (
        <IconButton label={t('downloads.pause', { name: item.fileName })} icon="pause" size="sm" clickAction={() => pause.run({ taskId: item.taskId })} />
      ) : null}
      {item.status === 'paused' ? (
        <IconButton label={t('downloads.resume', { name: item.fileName })} icon="play" size="sm" clickAction={() => resume.run({ taskId: item.taskId })} />
      ) : null}
      {isActive(item) ? (
        <IconButton label={t('downloads.cancel', { name: item.fileName })} icon="stop" size="sm" clickAction={() => cancel.run({ taskId: item.taskId })} />
      ) : null}
      {item.status === 'failed' ? (
        <Button label={t('downloads.retry')} size="sm" variant="secondary" clickAction={() => retry.run({ taskId: item.taskId })} />
      ) : null}
      {item.status === 'completed' ? (
        <IconButton label={t('downloads.open', { name: item.fileName })} icon="external-link" size="sm" clickAction={() => open.run({ taskId: item.taskId })} />
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

  return (
    <page data-testid="downloads-page" className="bg-surface text-text">
      <TopBar
        heading={t('downloads.title')}
        endContent={
          <view className="flex-row items-center gap-2">
            <Button
              label={t('downloads.clearCompleted')}
              variant="secondary"
              size="sm"
              isDisabled={!data.downloads.some((item) => !isActive(item))}
              clickAction={() => clearCompleted.run()}
            />
            <IconButton label={t('downloads.settings')} icon="settings" size="sm" clickAction={() => openSettings.run()} data-testid="downloads-settings-btn" />
          </view>
        }
        className="px-6"
      />
      <scroll className="grow bg-canvas">
        <view className="w-full max-w-[880px] self-center px-6 py-4 gap-4">
          <view className="flex-row items-center gap-3">
            <SegmentedControl
              label={t('downloads.filter')}
              bind={filter}
              items={FILTERS.map((value) => ({ value, label: t(`downloads.filter.${value}` as MessageKey) }))}
              size="sm"
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
          </view>
          {data.loading ? null : visible.length === 0 ? (
            <EmptyState
              title={data.downloads.length ? t('downloads.noMatches') : t('downloads.empty')}
              description={data.downloads.length ? undefined : t('downloads.emptyHint')}
              size="sm"
              data-testid="downloads-empty"
            />
          ) : (
            <ItemList label={t('downloads.title')}>
              {visible.map((item) => (
                <Item
                  key={item.taskId}
                  label={item.fileName}
                  description={description(item)}
                  startIcon={item.status === 'failed' ? 'error' : isActive(item) ? 'download' : 'file'}
                  endContent={controls(item)}
                  data-testid={`download-${item.taskId}`}
                />
              ))}
            </ItemList>
          )}
        </view>
      </scroll>
    </page>
  );
}
