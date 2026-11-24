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
  loading: boolean;
}

/** A toast for the View: a catalogue key and its count. */
/** A toast's text, translated here: the View's `t` works only while rendering. */
type Notice = { message: string };

const initialData: BookmarksData = {
  groups: [],
  entries: [],
  pinnedIds: [],
  selectedGroupId: '__all__',
  loading: true,
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

    return {
      async onLoad() {
        // System pages open in their own stack; the bar shows back only when a page is under them.
        void page.navigationBar.update({ homeButton: 'hidden' });
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
        /** Applies the edit form: only what changed goes to the store. */
        async updateBookmark({ id, title, groupId, pinned }: { id: string; title: string; groupId: string; pinned: boolean }) {
          const entry = page.data.entries.find((item) => item.id === id);
          if (!entry) return;
          const nextGroup = groupId.startsWith('__') ? undefined : groupId;
          if (title.trim() !== entry.title) await invoke('bookmarks.rename', { id, title: title.trim() });
          if (nextGroup !== entry.groupId) await invoke('bookmarks.move', { id, groupId: nextGroup });
          if (pinned !== page.data.pinnedIds.includes(id)) await invoke('bookmarks.setPinned', { id, pinned });
          await refresh();
        },
        async removeBookmark({ id }: { id: string }) {
          await invoke('bookmarks.remove', { id });
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
        async renameGroup({ id, name }: { id: string; name: string }) {
          const trimmed = name.trim();
          if (!trimmed) return;
          await invoke('bookmarks.renameGroup', { id, name: trimmed });
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
        async importHtml(): Promise<Notice | null> {
          const result = await invoke<{ imported: number } | null>('bookmarks.importHtml');
          if (!result) return null;
          await refresh();
          return { message: lx.app.t('bookmarks.status.imported', { count: result.imported }) };
        },
        async exportHtml(): Promise<Notice | null> {
          const result = await invoke<{ count: number } | null>('bookmarks.exportHtml');
          return result ? { message: lx.app.t('bookmarks.status.exported', { count: result.count }) } : null;
        },
        async openUrl({ url }: { url: string }) {
          await lx.surface.openUrl(url);
        },
      },
    };
  },
});
