import { List, t, useAction, useLocal, type MessageKey } from '@lingxia/view';
import { Badge } from '@liulidesign/core/Badge';
import { Button } from '@liulidesign/core/Button';
import { ContextMenu } from '@liulidesign/core/ContextMenu';
import { Dialog } from '@liulidesign/core/Dialog';
import { DropdownMenu, type DropdownMenuEntry } from '@liulidesign/core/DropdownMenu';
import { EmptyState } from '@liulidesign/core/EmptyState';
import { IconButton } from '@liulidesign/core/IconButton';
import { Item } from '@liulidesign/core/Item';
import { NavList } from '@liulidesign/core/NavList';
import { SearchField } from '@liulidesign/core/SearchField';
import { Select } from '@liulidesign/core/Select';
import { Switch } from '@liulidesign/core/Switch';
import { TextInput } from '@liulidesign/core/TextInput';
import { ToastRegion, useToast } from '@liulidesign/core/Toast';
import { TopBar } from '@liulidesign/core/TopBar';
import { hostOf } from '../../shared/format';
import { SiteIcon } from '../../shared/SiteIcon';
import type { View } from './$types';

const ALL = '__all__';
const UNGROUPED = '__ungrouped__';
// A fixed height lets the list place any of thousands of imported bookmarks without measuring them.
const ROW_HEIGHT = 56;

type Bookmark = View['data']['entries'][number];
type Notice = { message: string; count: number } | null;

export function render(view: View) {
  return (
    <page data-testid="bookmarks-page" className="bg-surface text-text">
      <ToastRegion>
        <Bookmarks {...view} />
      </ToastRegion>
    </page>
  );
}

function Bookmarks({ data, actions }: View) {
  const query = useLocal('query', '');
  // Which dialog is open: '' for none, 'add', 'group', 'renameGroup', 'deleteGroup', or `edit:<id>`.
  const dialog = useLocal('dialog', '');
  const formTitle = useLocal('formTitle', '');
  const formUrl = useLocal('formUrl', '');
  const formGroup = useLocal<string>('formGroup', UNGROUPED);
  const formPinned = useLocal('formPinned', false);
  const groupName = useLocal('groupName', '');

  const selectGroup = useAction(actions.selectGroup);
  const addBookmark = useAction(actions.addBookmark);
  const updateBookmark = useAction(actions.updateBookmark);
  const removeBookmark = useAction(actions.removeBookmark);
  const setPinned = useAction(actions.setPinned);
  const shiftBookmark = useAction(actions.shiftBookmark);
  const createGroup = useAction(actions.createGroup);
  const renameGroup = useAction(actions.renameGroup);
  const shiftGroup = useAction(actions.shiftGroup);
  const deleteGroup = useAction(actions.deleteGroup);
  const importHtml = useAction(actions.importHtml);
  const exportHtml = useAction(actions.exportHtml);
  const openUrl = useAction(actions.openUrl);
  const toast = useToast();

  const notify = async (outcome: Promise<{ ok: true; value: Notice } | { ok: false }>) => {
    const result = await outcome;
    if (result.ok && result.value) {
      toast.show({ message: t(result.value.message as MessageKey, { count: result.value.count }), tone: 'success' });
    }
  };

  const q = query.value.trim().toLowerCase();
  const visible = data.entries.filter((entry) => {
    if (data.selectedGroupId === UNGROUPED && entry.groupId) return false;
    if (!data.selectedGroupId.startsWith('__') && entry.groupId !== data.selectedGroupId) return false;
    return !q || entry.title.toLowerCase().includes(q) || entry.url.toLowerCase().includes(q);
  });
  const selectedGroup = data.groups.find((group) => group.id === data.selectedGroupId);
  const groupIndex = selectedGroup ? data.groups.indexOf(selectedGroup) : -1;
  // Order is kept per group, so reorder only while one group's list shows in full.
  const canReorder = data.selectedGroupId !== ALL && !q;
  const heading = selectedGroup?.name ?? (data.selectedGroupId === UNGROUPED ? t('bookmarks.ungrouped') : t('bookmarks.all'));
  const groupItems = [
    { value: UNGROUPED, label: t('bookmarks.ungrouped') },
    ...data.groups.map((group) => ({ value: group.id, label: group.name })),
  ];
  const editing = dialog.value.startsWith('edit:') ? data.entries.find((entry) => `edit:${entry.id}` === dialog.value) : undefined;

  const close = () => dialog.set('');
  const openAdd = () => {
    formTitle.set('');
    formUrl.set('');
    dialog.set('add');
  };
  const openEdit = (entry: Bookmark) => {
    formTitle.set(entry.title);
    formGroup.set(entry.groupId ?? UNGROUPED);
    formPinned.set(data.pinnedIds.includes(entry.id));
    dialog.set(`edit:${entry.id}`);
  };

  const rowMenu = (entry: Bookmark, index: number): DropdownMenuEntry[] => {
    const pinned = data.pinnedIds.includes(entry.id);
    return [
      { label: t('bookmarks.open'), icon: 'external-link', onClick: () => { void openUrl.run({ url: entry.url }); } },
      { label: t('bookmarks.edit'), icon: 'edit', onClick: () => openEdit(entry) },
      { label: pinned ? t('bookmarks.unpin') : t('bookmarks.pin'), icon: 'star', onClick: () => { void setPinned.run({ id: entry.id, pinned: !pinned }); } },
      ...(canReorder ? [
        { label: t('bookmarks.moveUp'), icon: 'arrow-up', isDisabled: index === 0, onClick: () => { void shiftBookmark.run({ id: entry.id, delta: -1 }); } },
        { label: t('bookmarks.moveDown'), icon: 'arrow-down', isDisabled: index === visible.length - 1, onClick: () => { void shiftBookmark.run({ id: entry.id, delta: 1 }); } },
      ] satisfies DropdownMenuEntry[] : []),
      { type: 'divider' },
      { label: t('common.delete'), icon: 'delete', variant: 'destructive', onClick: () => { void removeBookmark.run({ id: entry.id }); } },
    ];
  };

  const renderRow = (entry: Bookmark, index: number) => {
    const title = entry.title || entry.url;
    return (
      <view className="h-14">
      <ContextMenu label={title} items={rowMenu(entry, index)} data-testid={`bookmark-${entry.id}`}>
        <Item
          label={title}
          labelEnd={data.pinnedIds.includes(entry.id) ? <Badge label={t('bookmarks.pinned')} size="sm" /> : undefined}
          description={entry.url}
          descriptionLines={1}
          startContent={<SiteIcon host={hostOf(entry.url)} />}
          density="compact"
          onClick={() => { void openUrl.run({ url: entry.url }); }}
          endAction={{ label: t('bookmarks.editNamed', { title }), icon: 'more', onClick: () => openEdit(entry), 'data-testid': `bookmark-edit-${entry.id}` }}
          className="h-14 rounded-control"
        />
      </ContextMenu>
      </view>
    );
  };

  const groupMenu: DropdownMenuEntry[] = selectedGroup ? [
    { label: t('bookmarks.renameGroup'), icon: 'edit', onClick: () => { groupName.set(selectedGroup.name); dialog.set('renameGroup'); } },
    { label: t('bookmarks.moveGroupUp'), icon: 'arrow-up', isDisabled: groupIndex <= 0, onClick: () => { void shiftGroup.run({ id: selectedGroup.id, delta: -1 }); } },
    { label: t('bookmarks.moveGroupDown'), icon: 'arrow-down', isDisabled: groupIndex >= data.groups.length - 1, onClick: () => { void shiftGroup.run({ id: selectedGroup.id, delta: 1 }); } },
    { type: 'divider' },
    { label: t('bookmarks.deleteGroup'), icon: 'delete', variant: 'destructive', onClick: () => dialog.set('deleteGroup') },
  ] : [];

  return (
    <view className="grow min-h-0">
      <TopBar
        heading={t('bookmarks.title')}
        endContent={
          <view className="flex-row items-center gap-2">
            <Button label={t('bookmarks.add')} variant="primary" size="sm" startIcon="add" onClick={openAdd} data-testid="bookmarks-add" />
            <DropdownMenu
              label={t('bookmarks.more')}
              size="sm"
              alignment="end"
              items={[
                { label: t('bookmarks.import'), icon: 'upload', onClick: () => { void notify(importHtml.run()); } },
                { label: t('bookmarks.export'), icon: 'download', onClick: () => { void notify(exportHtml.run()); } },
              ]}
            >
              <IconButton label={t('bookmarks.more')} icon="more" size="sm" edge="end" data-testid="bookmarks-more" />
            </DropdownMenu>
          </view>
        }
        className="px-6"
      />
      <view className="grow min-h-0 flex-row bg-canvas">
        <view className="w-[240px] shrink-0 px-3 py-4 gap-2 border-e border-text/8">
          <NavList
            label={t('bookmarks.title')}
            sections={[
              { id: 'library', items: [
                { value: ALL, label: t('bookmarks.all'), endText: String(data.entries.length) },
                { value: UNGROUPED, label: t('bookmarks.ungrouped') },
              ] },
              ...(data.groups.length ? [{ id: 'groups', title: t('bookmarks.groups'), items: data.groups.map((group) => ({
                value: group.id,
                label: group.name,
                endText: String(data.entries.filter((entry) => entry.groupId === group.id).length),
              })) }] : []),
            ]}
            value={data.selectedGroupId}
            onChange={(groupId) => { void selectGroup.run({ groupId }); }}
            data-testid="bookmarks-groups"
          />
          <view className="flex-row">
            <Button label={t('bookmarks.newGroup')} variant="ghost" size="sm" startIcon="add" onClick={() => { groupName.set(''); dialog.set('group'); }} data-testid="bookmarks-new-group" />
          </view>
        </view>

        <view className="grow basis-0 min-w-0 min-h-0 px-6 pt-4 gap-3">
          <view className="flex-row items-center gap-2 min-h-control-md">
            <text role="heading" headingLevel={2} className="grow shrink basis-0 min-w-0 truncate text-title font-semibold">{heading}</text>
            {selectedGroup ? (
              <DropdownMenu label={t('bookmarks.groupActions', { name: selectedGroup.name })} size="sm" alignment="end" items={groupMenu}>
                <IconButton label={t('bookmarks.groupActions', { name: selectedGroup.name })} icon="more" size="sm" edge="end" data-testid="bookmarks-group-more" />
              </DropdownMenu>
            ) : null}
          </view>
          <SearchField label={t('bookmarks.search')} isLabelHidden placeholder={t('bookmarks.search')} bind={query} data-testid="bookmarks-search" />
          {data.loading ? null : visible.length === 0 ? (
            <EmptyState
              title={q ? t('bookmarks.noMatches') : t('bookmarks.empty')}
              description={q ? undefined : t('bookmarks.emptyHint')}
              actions={q ? undefined : <Button label={t('bookmarks.add')} variant="secondary" startIcon="add" onClick={openAdd} />}
              className="grow justify-center"
              data-testid="bookmarks-empty"
            />
          ) : (
            <List
              items={visible}
              getKey={(entry) => entry.id}
              getItemHeight={() => ROW_HEIGHT}
              renderItem={renderRow}
              className="grow pb-6"
              data-testid="bookmarks-list"
            />
          )}
        </view>
      </view>

      <Dialog
        isOpen={dialog.value === 'add'}
        onOpenChange={(open) => { if (!open) close(); }}
        label={t('bookmarks.add')}
        size="sm"
        data-testid="bookmarks-add-dialog"
        actions={
          <>
            <Button label={t('common.cancel')} onClick={close} />
            <Button
              label={t('common.save')}
              variant="primary"
              isDisabled={!formUrl.value.trim()}
              clickAction={async () => {
                const result = await addBookmark.run({ title: formTitle.value, url: formUrl.value, groupId: data.selectedGroupId });
                if (result.ok) close();
              }}
              data-testid="bookmarks-add-save"
            />
          </>
        }
      >
        <view className="gap-4">
          <TextInput label={t('bookmarks.url')} bind={formUrl} type="text" inputMode="text" autoCapitalize="none" placeholder="https://" hasAutoFocus isRequired />
          <TextInput label={t('bookmarks.name')} bind={formTitle} optionalLabel={t('common.optional')} />
        </view>
      </Dialog>

      <Dialog
        isOpen={editing !== undefined}
        onOpenChange={(open) => { if (!open) close(); }}
        label={t('bookmarks.edit')}
        description={editing?.url}
        size="sm"
        data-testid="bookmarks-edit-dialog"
        actions={
          <>
            <Button
              label={t('common.delete')}
              variant="ghost"
              className="me-auto"
              clickAction={async () => { if (editing) await removeBookmark.run({ id: editing.id }); close(); }}
            />
            <Button label={t('common.cancel')} onClick={close} />
            <Button
              label={t('common.save')}
              variant="primary"
              clickAction={async () => {
                if (!editing) return;
                const result = await updateBookmark.run({ id: editing.id, title: formTitle.value, groupId: formGroup.value, pinned: formPinned.value });
                if (result.ok) close();
              }}
              data-testid="bookmarks-edit-save"
            />
          </>
        }
      >
        <view className="gap-4">
          <TextInput label={t('bookmarks.name')} bind={formTitle} placeholder={editing?.url} />
          {data.groups.length ? <Select label={t('bookmarks.group')} bind={formGroup} items={groupItems} /> : null}
          <Switch label={t('bookmarks.pinToSidebar')} bind={formPinned} />
        </view>
      </Dialog>

      <Dialog
        isOpen={dialog.value === 'group' || dialog.value === 'renameGroup'}
        onOpenChange={(open) => { if (!open) close(); }}
        label={dialog.value === 'group' ? t('bookmarks.newGroup') : t('bookmarks.renameGroup')}
        size="sm"
        actions={
          <>
            <Button label={t('common.cancel')} onClick={close} />
            <Button
              label={t('common.save')}
              variant="primary"
              isDisabled={!groupName.value.trim()}
              clickAction={async () => {
                const result = dialog.value === 'group' || !selectedGroup
                  ? await createGroup.run({ name: groupName.value })
                  : await renameGroup.run({ id: selectedGroup.id, name: groupName.value });
                if (result.ok) close();
              }}
              data-testid="bookmarks-group-save"
            />
          </>
        }
      >
        <TextInput label={t('bookmarks.groupName')} bind={groupName} hasAutoFocus />
      </Dialog>

      <Dialog
        isOpen={dialog.value === 'deleteGroup' && selectedGroup !== undefined}
        onOpenChange={(open) => { if (!open) close(); }}
        label={t('bookmarks.deleteGroupConfirm', { name: selectedGroup?.name ?? '' })}
        description={t('bookmarks.deleteGroupHint')}
        size="sm"
        isAlert
        dismissOn="closeRequest"
        actions={
          <>
            <Button label={t('common.cancel')} onClick={close} />
            <Button
              label={t('bookmarks.deleteGroup')}
              variant="destructive"
              clickAction={async () => { if (selectedGroup) await deleteGroup.run({ id: selectedGroup.id }); close(); }}
            />
          </>
        }
      />
    </view>
  );
}
