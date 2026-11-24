import { invoke, watch } from '../../shared/native';
import { definePage, lx } from '@lingxia/logic';

interface BookmarkGroup {
  id: string;
  name: string;
}

interface BookmarkEntry {
  id: string;
  url: string;
  title: string;
  groupId?: string;
  createdAtMs: number;
}

interface BookmarksView {
  groups: BookmarkGroup[];
  entries: BookmarkEntry[];
  pinnedIds: string[];
}

interface BookmarksData {
  groups: BookmarkGroup[];
  entries: BookmarkEntry[];
  pinnedIds: string[];
  selectedGroupId: string;
  editingId: string;
  editingGroupId: string;
  loading: boolean;
  /** An i18n key the View renders; empty when there is nothing to say. */
  status: string;
  statusCount: number;
}

const initialData: BookmarksData = {
  groups: [],
  entries: [],
  pinnedIds: [],
  selectedGroupId: '__all__',
  editingId: '',
  editingGroupId: '',
  loading: true,
  status: '',
  statusCount: 0,
};

export default definePage({
  data: initialData,
  setup(page) {
    function apply(view: BookmarksView) {
      page.update((d) => {
        d.groups = view.groups;
        d.entries = view.entries;
        d.pinnedIds = view.pinnedIds;
        if (d.selectedGroupId !== '__all__' && d.selectedGroupId !== '__ungrouped__'
          && !view.groups.some((group) => group.id === d.selectedGroupId)) {
          d.selectedGroupId = '__all__';
        }
        d.loading = false;
      });
    }

    async function refresh() {
      apply(await invoke<BookmarksView>('bookmarks.list'));
    }

    /** The ids of `ids` with `id` swapped one step by `delta`, or null at an end. */
    function shifted(ids: string[], id: string, delta: -1 | 1): string[] | null {
      const from = ids.indexOf(id);
      const to = from + delta;
      if (from < 0 || to < 0 || to >= ids.length) return null;
      const next = ids.slice();
      [next[from], next[to]] = [next[to], next[from]];
      return next;
    }

    function report(status: string, count = 0) {
      page.update((d) => {
        d.status = status;
        d.statusCount = count;
      });
    }

    return {
      async onLoad() {
        watch<BookmarksView>('bookmarks.watch', page.signal, apply);
        await refresh().catch(() => page.update((d) => { d.loading = false; }));
      },
      actions: {
        async selectGroup({ groupId }: { groupId: string }) {
          page.update((d) => {
            d.selectedGroupId = groupId;
            d.editingGroupId = '';
          });
        },
        async addBookmark({ title, url, groupId }: { title: string; url: string; groupId: string }) {
          await invoke('bookmarks.add', {
            url: url.trim(),
            title: title.trim() || undefined,
            groupId: groupId.startsWith('__') ? undefined : groupId,
          });
          await refresh();
        },
        async startRename({ id }: { id: string }) {
          page.update((d) => { d.editingId = id; });
        },
        async renameBookmark({ id, title }: { id: string; title: string }) {
          await invoke('bookmarks.rename', { id, title: title.trim() });
          page.update((d) => { d.editingId = ''; });
          await refresh();
        },
        async removeBookmark({ id }: { id: string }) {
          await invoke('bookmarks.remove', { id });
          await refresh();
        },
        async moveBookmark({ id, groupId }: { id: string; groupId: string }) {
          await invoke('bookmarks.move', { id, groupId: groupId.startsWith('__') ? undefined : groupId });
          await refresh();
        },
        async shiftBookmark({ id, delta }: { id: string; delta: -1 | 1 }) {
          const entry = page.data.entries.find((item) => item.id === id);
          if (!entry) return;
          // The route reorders one group at a time: the entry's own group.
          const scope = page.data.entries
            .filter((item) => item.groupId === entry.groupId)
            .map((item) => item.id);
          const orderedIds = shifted(scope, id, delta);
          if (!orderedIds) return;
          await invoke('bookmarks.reorder', { groupId: entry.groupId, orderedIds });
          await refresh();
        },
        async setPinned({ id, pinned }: { id: string; pinned: boolean }) {
          await invoke('bookmarks.setPinned', { id, pinned });
          await refresh();
        },
        async createGroup({ name }: { name: string }) {
          const trimmed = name.trim();
          if (!trimmed) return;
          await invoke('bookmarks.createGroup', { name: trimmed });
          await refresh();
        },
        async startGroupRename({ id }: { id: string }) {
          page.update((d) => { d.editingGroupId = id; });
        },
        async renameGroup({ id, name }: { id: string; name: string }) {
          const trimmed = name.trim();
          if (!trimmed) return;
          await invoke('bookmarks.renameGroup', { id, name: trimmed });
          page.update((d) => { d.editingGroupId = ''; });
          await refresh();
        },
        async shiftGroup({ id, delta }: { id: string; delta: -1 | 1 }) {
          const orderedIds = shifted(page.data.groups.map((group) => group.id), id, delta);
          if (!orderedIds) return;
          await invoke('bookmarks.reorderGroups', { orderedIds });
          await refresh();
        },
        async deleteGroup({ id }: { id: string }) {
          await invoke('bookmarks.deleteGroup', { id });
          await refresh();
        },
        async importHtml() {
          const result = await invoke<{ imported: number } | null>('bookmarks.importHtml');
          if (!result) return;
          await refresh();
          report('bookmarks.status.imported', result.imported);
        },
        async exportHtml() {
          const result = await invoke<{ count: number } | null>('bookmarks.exportHtml');
          if (result) report('bookmarks.status.exported', result.count);
        },
        async openUrl({ url }: { url: string }) {
          await lx.surface.openUrl(url);
        },
      },
    };
  },
});
