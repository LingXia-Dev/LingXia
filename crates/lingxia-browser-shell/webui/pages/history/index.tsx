import { t, useAction, useLocal } from '@lingxia/view';
import { formatTime } from '../../shared/format';
import type { View } from './$types';

export function render({ data, actions }: View) {
  const search = useLocal('search', '');
  const removeEntry = useAction(actions.removeEntry);
  const clearRange = useAction(actions.clearRange);
  const openUrl = useAction(actions.openUrl);

  const q = search.value.trim().toLowerCase();
  const visible = data.entries.filter((entry) => {
    if (!q) return true;
    return entry.title.toLowerCase().includes(q) || entry.url.toLowerCase().includes(q);
  });

  return (
    <page data-testid="history-page" className="bg-canvas text-text px-6 py-6 gap-4">
      <view className="flex-row items-center justify-between gap-4">
        <text className="text-2xl font-semibold">{t('history.title')}</text>
        <view className="flex-row items-center gap-2">
          <button
            onClick={() => { void clearRange.run({ sinceMs: Date.now() - 86_400_000 }); }}
            className="px-3 py-1.5 rounded-lg bg-surface border border-line text-sm"
          >
            <text>{t('history.clearDay')}</text>
          </button>
          <button
            onClick={() => { void clearRange.run({}); }}
            className="px-3 py-1.5 rounded-lg bg-danger-soft text-danger text-sm"
          >
            <text>{t('history.clearAll')}</text>
          </button>
        </view>
      </view>

      <input
        aria-label={t('common.search')}
        value={search.value}
        placeholder={t('common.search')}
        onInput={(v) => search.set(v)}
        className="px-3 py-2 rounded-lg bg-surface border border-line text-sm"
      />

      <scroll className="grow basis-0 min-h-0 rounded-xl bg-surface border border-line">
        {visible.length === 0 ? (
          <view className="p-6 items-center">
            <text className="text-sm text-muted">{data.loading ? t('common.loading') : t('common.empty')}</text>
          </view>
        ) : (
          visible.map((entry) => (
            <view key={entry.id} className="flex-row items-center justify-between px-4 py-3 border-t border-line gap-3">
              <button
                onClick={() => { void openUrl.run({ url: entry.url }); }}
                className="grow basis-0 min-w-0 bg-transparent gap-0.5"
              >
                <text className="text-sm font-medium">{entry.title || entry.url}</text>
                <text className="text-xs text-muted">{entry.url}</text>
              </button>
              <text className="text-xs text-muted">{formatTime(entry.visitedAtMs)}</text>
              <button
                onClick={() => { void removeEntry.run({ id: entry.id }); }}
                className="px-2.5 py-1 rounded-md bg-transparent text-xs text-danger"
              >
                <text>{t('common.delete')}</text>
              </button>
            </view>
          ))
        )}
      </scroll>
    </page>
  );
}
