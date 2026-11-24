import { t, useAction, useLocal, type MessageKey } from '@lingxia/view';
import type { View } from './$types';

export function render({ data, actions }: View) {
  const search = useLocal('search', '');
  const newTitle = useLocal('newTitle', '');
  const newUrl = useLocal('newUrl', '');
  const newGroup = useLocal('newGroup', '');
  const renameTitle = useLocal('renameTitle', '');
  const groupName = useLocal('groupName', '');

  const selectGroup = useAction(actions.selectGroup);
  const addBookmark = useAction(actions.addBookmark);
  const removeBookmark = useAction(actions.removeBookmark);
  const setPinned = useAction(actions.setPinned);
  const startRename = useAction(actions.startRename);
  const renameBookmark = useAction(actions.renameBookmark);
  const moveBookmark = useAction(actions.moveBookmark);
  const shiftBookmark = useAction(actions.shiftBookmark);
  const createGroup = useAction(actions.createGroup);
  const startGroupRename = useAction(actions.startGroupRename);
  const renameGroup = useAction(actions.renameGroup);
  const shiftGroup = useAction(actions.shiftGroup);
  const deleteGroup = useAction(actions.deleteGroup);
  const importHtml = useAction(actions.importHtml);
  const exportHtml = useAction(actions.exportHtml);
  const openUrl = useAction(actions.openUrl);

  const q = search.value.trim().toLowerCase();
  const visible = data.entries.filter((entry) => {
    if (data.selectedGroupId === '__ungrouped__' && entry.groupId) return false;
    if (!data.selectedGroupId.startsWith('__') && entry.groupId !== data.selectedGroupId) return false;
    if (!q) return true;
    return entry.title.toLowerCase().includes(q) || entry.url.toLowerCase().includes(q);
  });

  const selectedGroup = data.groups.find((group) => group.id === data.selectedGroupId);
  const groupIndex = selectedGroup ? data.groups.indexOf(selectedGroup) : -1;
  // Order is kept per group, so reorder only while one group's list shows in full.
  const canReorder = data.selectedGroupId !== '__all__' && !q;
  const moveOptions = [
    { value: '__ungrouped__', label: t('bookmarks.ungrouped') },
    ...data.groups.map((group) => ({ value: group.id, label: group.name })),
  ];
  const stepClass = (enabled: boolean) => (
    enabled
      ? 'px-2 py-1 rounded-md bg-canvas border border-line text-xs'
      : 'px-2 py-1 rounded-md bg-canvas border border-line text-xs opacity-40'
  );

  const submitAdd = async () => {
    if (!newUrl.value.trim()) return;
    const res = await addBookmark.run({
      title: newTitle.value,
      url: newUrl.value,
      groupId: data.selectedGroupId,
    });
    if (res.ok) {
      newTitle.set('');
      newUrl.set('');
    }
  };

  const submitGroup = async () => {
    if (!newGroup.value.trim()) return;
    const res = await createGroup.run({ name: newGroup.value });
    if (res.ok) newGroup.set('');
  };

  return (
    <page data-testid="bookmarks-page" className="bg-canvas text-text px-6 py-6 gap-4">
      <view className="flex-row items-center justify-between gap-4">
        <text className="text-2xl font-semibold">{t('bookmarks.title')}</text>
        <view className="flex-row items-center gap-2">
          {data.status ? <text className="text-xs text-accent">{t(data.status as MessageKey, { count: data.statusCount })}</text> : null}
          <button
            onClick={() => { void importHtml.run(); }}
            className="px-3 py-1.5 rounded-lg bg-surface border border-line text-sm"
          >
            <text>{t('bookmarks.import')}</text>
          </button>
          <button
            onClick={() => { void exportHtml.run(); }}
            className="px-3 py-1.5 rounded-lg bg-surface border border-line text-sm"
          >
            <text>{t('bookmarks.export')}</text>
          </button>
        </view>
      </view>

      <view className="flex-row gap-4 grow basis-0 min-h-0">
        <view className="w-56 gap-2">
          <button
            onClick={() => { void selectGroup.run({ groupId: '__all__' }); }}
            className={data.selectedGroupId === '__all__' ? 'px-3 py-2 rounded-lg bg-accent text-on-accent text-sm' : 'px-3 py-2 rounded-lg bg-surface text-sm'}
          >
            <text>{t('bookmarks.all')}</text>
          </button>
          <button
            onClick={() => { void selectGroup.run({ groupId: '__ungrouped__' }); }}
            className={data.selectedGroupId === '__ungrouped__' ? 'px-3 py-2 rounded-lg bg-accent text-on-accent text-sm' : 'px-3 py-2 rounded-lg bg-surface text-sm'}
          >
            <text>{t('bookmarks.ungrouped')}</text>
          </button>
          {data.groups.map((group) => (
            <button
              key={group.id}
              onClick={() => { void selectGroup.run({ groupId: group.id }); }}
              className={data.selectedGroupId === group.id ? 'px-3 py-2 rounded-lg bg-accent text-on-accent text-sm' : 'px-3 py-2 rounded-lg bg-surface text-sm'}
            >
              <text>{group.name}</text>
            </button>
          ))}
          <view className="flex-row gap-1 pt-2">
            <input
              aria-label={t('bookmarks.newGroup')}
              value={newGroup.value}
              placeholder={t('bookmarks.newGroup')}
              onInput={(v) => newGroup.set(v)}
              onSubmit={() => { void submitGroup(); }}
              className="grow basis-0 min-w-0 px-2.5 py-1.5 rounded-lg bg-surface border border-line text-xs"
            />
            <button onClick={() => { void submitGroup(); }} className="px-2.5 py-1.5 rounded-lg bg-surface border border-line text-xs">
              <text>{t('bookmarks.addGroup')}</text>
            </button>
          </view>
        </view>

        <view className="grow basis-0 min-w-0 gap-3">
          {selectedGroup ? (
            <view className="flex-row items-center gap-2">
              {data.editingGroupId === selectedGroup.id ? (
                <input
                  aria-label={t('bookmarks.renameGroup')}
                  value={groupName.value}
                  autoFocus
                  onInput={(v) => groupName.set(v)}
                  onSubmit={(v) => { void renameGroup.run({ id: selectedGroup.id, name: v }); }}
                  className="grow basis-0 min-w-0 px-3 py-1.5 rounded-lg bg-surface border border-line text-sm"
                />
              ) : (
                <text className="grow basis-0 min-w-0 text-base font-semibold">{selectedGroup.name}</text>
              )}
              {data.editingGroupId === selectedGroup.id ? (
                <button
                  onClick={() => { void renameGroup.run({ id: selectedGroup.id, name: groupName.value }); }}
                  className="px-3 py-1.5 rounded-lg bg-accent text-on-accent text-sm"
                >
                  <text>{t('common.save')}</text>
                </button>
              ) : (
                <button
                  onClick={() => { groupName.set(selectedGroup.name); void startGroupRename.run({ id: selectedGroup.id }); }}
                  className="px-3 py-1.5 rounded-lg bg-surface border border-line text-sm"
                >
                  <text>{t('bookmarks.renameGroup')}</text>
                </button>
              )}
              <button
                disabled={groupIndex <= 0}
                onClick={() => { void shiftGroup.run({ id: selectedGroup.id, delta: -1 }); }}
                className={groupIndex <= 0 ? 'px-3 py-1.5 rounded-lg bg-surface border border-line text-sm opacity-40' : 'px-3 py-1.5 rounded-lg bg-surface border border-line text-sm'}
              >
                <text>{t('bookmarks.moveGroupUp')}</text>
              </button>
              <button
                disabled={groupIndex >= data.groups.length - 1}
                onClick={() => { void shiftGroup.run({ id: selectedGroup.id, delta: 1 }); }}
                className={groupIndex >= data.groups.length - 1 ? 'px-3 py-1.5 rounded-lg bg-surface border border-line text-sm opacity-40' : 'px-3 py-1.5 rounded-lg bg-surface border border-line text-sm'}
              >
                <text>{t('bookmarks.moveGroupDown')}</text>
              </button>
              <button
                onClick={() => { void deleteGroup.run({ id: selectedGroup.id }); }}
                className="px-3 py-1.5 rounded-lg bg-transparent text-sm text-danger"
              >
                <text>{t('bookmarks.deleteGroup')}</text>
              </button>
            </view>
          ) : null}

          <view className="flex-row gap-2">
            <input
              aria-label={t('common.search')}
              value={search.value}
              placeholder={t('common.search')}
              onInput={(v) => search.set(v)}
              className="grow basis-0 min-w-0 px-3 py-2 rounded-lg bg-surface border border-line text-sm"
            />
          </view>

          <view className="flex-row gap-2">
            <input
              aria-label={t('bookmarks.addTitle')}
              value={newTitle.value}
              placeholder={t('bookmarks.addTitle')}
              onInput={(v) => newTitle.set(v)}
              className="w-44 px-3 py-2 rounded-lg bg-surface border border-line text-sm"
            />
            <input
              aria-label={t('bookmarks.addUrl')}
              value={newUrl.value}
              placeholder={t('bookmarks.addUrl')}
              onInput={(v) => newUrl.set(v)}
              onSubmit={() => { void submitAdd(); }}
              className="grow basis-0 min-w-0 px-3 py-2 rounded-lg bg-surface border border-line text-sm"
            />
            <button
              onClick={() => { void submitAdd(); }}
              className="px-4 py-2 rounded-lg bg-accent text-on-accent text-sm font-medium"
            >
              <text>{t('bookmarks.add')}</text>
            </button>
          </view>

          <scroll className="grow basis-0 min-h-0 rounded-xl bg-surface border border-line">
            {visible.length === 0 ? (
              <view className="p-6 items-center">
                <text className="text-sm text-muted">{data.loading ? t('common.loading') : t('common.empty')}</text>
              </view>
            ) : (
              visible.map((entry, index) => (
                <view key={entry.id} className="flex-row items-center justify-between px-4 py-3 border-t border-line gap-3">
                  {data.editingId === entry.id ? (
                    <input
                      aria-label={t('bookmarks.rename')}
                      value={renameTitle.value}
                      placeholder={entry.url}
                      autoFocus
                      onInput={(v) => renameTitle.set(v)}
                      onSubmit={(v) => { void renameBookmark.run({ id: entry.id, title: v }); }}
                      className="grow basis-0 min-w-0 px-3 py-1.5 rounded-lg bg-canvas border border-line text-sm"
                    />
                  ) : (
                    <button
                      onClick={() => { void openUrl.run({ url: entry.url }); }}
                      className="grow basis-0 min-w-0 bg-transparent gap-0.5"
                    >
                      <text className="text-sm font-medium">{entry.title || entry.url}</text>
                      <text className="text-xs text-muted">{entry.url}</text>
                    </button>
                  )}
                  <view className="flex-row items-center gap-2">
                    {canReorder ? (
                      <button
                        aria-label={t('bookmarks.moveUp')}
                        disabled={index === 0}
                        onClick={() => { void shiftBookmark.run({ id: entry.id, delta: -1 }); }}
                        className={stepClass(index > 0)}
                      >
                        <text>↑</text>
                      </button>
                    ) : null}
                    {canReorder ? (
                      <button
                        aria-label={t('bookmarks.moveDown')}
                        disabled={index === visible.length - 1}
                        onClick={() => { void shiftBookmark.run({ id: entry.id, delta: 1 }); }}
                        className={stepClass(index < visible.length - 1)}
                      >
                        <text>↓</text>
                      </button>
                    ) : null}
                    {data.groups.length > 0 ? (
                      <select
                        aria-label={t('bookmarks.moveTo')}
                        value={entry.groupId ?? '__ungrouped__'}
                        options={moveOptions}
                        onChange={(value) => { void moveBookmark.run({ id: entry.id, groupId: value }); }}
                      />
                    ) : null}
                    {data.editingId === entry.id ? (
                      <button
                        onClick={() => { void renameBookmark.run({ id: entry.id, title: renameTitle.value }); }}
                        className="px-2.5 py-1 rounded-md bg-accent text-on-accent text-xs"
                      >
                        <text>{t('common.save')}</text>
                      </button>
                    ) : (
                      <button
                        onClick={() => { renameTitle.set(entry.title); void startRename.run({ id: entry.id }); }}
                        className="px-2.5 py-1 rounded-md bg-canvas border border-line text-xs"
                      >
                        <text>{t('bookmarks.rename')}</text>
                      </button>
                    )}
                    <button
                      onClick={() => { void setPinned.run({ id: entry.id, pinned: !data.pinnedIds.includes(entry.id) }); }}
                      className="px-2.5 py-1 rounded-md bg-canvas border border-line text-xs"
                    >
                      <text>{data.pinnedIds.includes(entry.id) ? t('bookmarks.unpin') : t('bookmarks.pin')}</text>
                    </button>
                    <button
                      onClick={() => { void removeBookmark.run({ id: entry.id }); }}
                      className="px-2.5 py-1 rounded-md bg-transparent text-xs text-danger"
                    >
                      <text>{t('common.delete')}</text>
                    </button>
                  </view>
                </view>
              ))
            )}
          </scroll>
        </view>
      </view>
    </page>
  );
}
