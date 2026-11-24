import { t, useAction, useLocal, type MessageKey } from '@lingxia/view';
import { formatBytes } from '../../shared/format';
import type { View } from './$types';

export function render({ data, actions }: View) {
  const search = useLocal('search', '');
  const setFilter = useAction(actions.setFilter);
  const pause = useAction(actions.pause);
  const resume = useAction(actions.resume);
  const retry = useAction(actions.retry);
  const cancel = useAction(actions.cancel);
  const remove = useAction(actions.remove);
  const open = useAction(actions.open);
  const reveal = useAction(actions.reveal);
  const clearCompleted = useAction(actions.clearCompleted);
  const openSettings = useAction(actions.openSettings);

  const q = search.value.trim().toLowerCase();
  const visible = data.downloads.filter((item) => {
    if (data.filter === 'active' && item.status !== 'downloading' && item.status !== 'paused') return false;
    if (data.filter === 'completed' && item.status !== 'completed') return false;
    if (!q) return true;
    return item.fileName.toLowerCase().includes(q) || item.url.toLowerCase().includes(q);
  });

  return (
    <page data-testid="downloads-page" className="bg-canvas text-text px-6 py-6 gap-4">
      <view className="flex-row items-center justify-between gap-4">
        <text className="text-2xl font-semibold">{t('downloads.title')}</text>
        <view className="flex-row items-center gap-2">
          <button
            onClick={() => { void clearCompleted.run(); }}
            className="px-3 py-1.5 rounded-lg bg-surface border border-line text-sm"
          >
            <text>{t('downloads.clearCompleted')}</text>
          </button>
          <button
            data-testid="downloads-settings-btn"
            onClick={() => { void openSettings.run(); }}
            className="px-3 py-1.5 rounded-lg bg-surface border border-line text-sm"
          >
            <text>{t('nav.settings')}</text>
          </button>
        </view>
      </view>

      <view className="flex-row items-center gap-2">
        {(['all', 'active', 'completed'] as const).map((f) => (
          <button
            key={f}
            onClick={() => { void setFilter.run({ filter: f }); }}
            className={data.filter === f ? 'px-3 py-1.5 rounded-lg bg-accent text-on-accent text-sm' : 'px-3 py-1.5 rounded-lg bg-surface border border-line text-sm'}
          >
            <text>{t(`downloads.${f}` as MessageKey)}</text>
          </button>
        ))}
        <input
          aria-label={t('common.search')}
          value={search.value}
          placeholder={t('common.search')}
          onInput={(v) => search.set(v)}
          className="grow basis-0 min-w-0 px-3 py-1.5 rounded-lg bg-surface border border-line text-sm"
        />
      </view>

      <scroll className="grow basis-0 min-h-0 rounded-xl bg-surface border border-line">
        {visible.length === 0 ? (
          <view className="p-6 items-center">
            <text className="text-sm text-muted">{data.loading ? t('common.loading') : t('common.empty')}</text>
          </view>
        ) : (
          visible.map((item) => (
            <view key={item.taskId} className="px-4 py-3 border-t border-line gap-2">
              <view className="flex-row items-center justify-between gap-3">
                <view className="grow basis-0 min-w-0 gap-0.5">
                  <text className="text-sm font-medium">{item.fileName}</text>
                  <text className="text-xs text-muted">
                    {formatBytes(item.downloadedBytes)}
                    {item.totalBytes ? ` / ${formatBytes(item.totalBytes)}` : ''}
                    {` · ${t(`downloads.status.${item.status}` as MessageKey)}`}
                  </text>
                </view>
                <view className="flex-row items-center gap-2">
                  {item.status === 'downloading' ? (
                    <button onClick={() => { void pause.run({ taskId: item.taskId }); }} className="px-2.5 py-1 rounded-md bg-canvas border border-line text-xs">
                      <text>{t('downloads.pause')}</text>
                    </button>
                  ) : null}
                  {item.status === 'paused' ? (
                    <button onClick={() => { void resume.run({ taskId: item.taskId }); }} className="px-2.5 py-1 rounded-md bg-canvas border border-line text-xs">
                      <text>{t('downloads.resume')}</text>
                    </button>
                  ) : null}
                  {item.status === 'failed' ? (
                    <button onClick={() => { void retry.run({ taskId: item.taskId }); }} className="px-2.5 py-1 rounded-md bg-canvas border border-line text-xs">
                      <text>{t('downloads.retry')}</text>
                    </button>
                  ) : null}
                  {item.status === 'completed' ? (
                    <view className="flex-row items-center gap-2">
                      <button onClick={() => { void open.run({ taskId: item.taskId }); }} className="px-2.5 py-1 rounded-md bg-accent text-on-accent text-xs">
                        <text>{t('common.open')}</text>
                      </button>
                      <button onClick={() => { void reveal.run({ taskId: item.taskId }); }} className="px-2.5 py-1 rounded-md bg-canvas border border-line text-xs">
                        <text>{t('downloads.reveal')}</text>
                      </button>
                    </view>
                  ) : (
                    <button onClick={() => { void cancel.run({ taskId: item.taskId }); }} className="px-2.5 py-1 rounded-md bg-transparent text-xs text-danger">
                      <text>{t('common.cancel')}</text>
                    </button>
                  )}
                  <button onClick={() => { void remove.run({ taskId: item.taskId }); }} className="px-2.5 py-1 rounded-md bg-transparent text-xs text-danger">
                    <text>{t('common.delete')}</text>
                  </button>
                </view>
              </view>
            </view>
          ))
        )}
      </scroll>
    </page>
  );
}
