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
          page.update((d) => { d.selectedGroupId = groupId; });
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
