import { browserNative } from '../../shared/native';
import { definePage, lx } from '@lingxia/logic';

interface BookmarkGroup {
  id: string;
  name: string;
}

interface BookmarkEntry {
  id: string;
  title: string;
  url: string;
  groupId: string | null;
  pinned?: boolean;
}

interface BookmarksSnapshot {
  groups: BookmarkGroup[];
  entries: BookmarkEntry[];
}

interface BookmarksData {
  groups: BookmarkGroup[];
  entries: BookmarkEntry[];
  selectedGroupId: string;
  loading: boolean;
  statusMessage: string;
}

const initialData: BookmarksData = {
  groups: [],
  entries: [],
  selectedGroupId: '__all__',
  loading: true,
  statusMessage: '',
};

export default definePage({
  data: initialData,
  setup(page) {
    const native = browserNative(page);
    let disposed = false;
    let unwatch: (() => void) | null = null;

    function applySnapshot(snap: BookmarksSnapshot) {
      if (disposed) return;
      page.update((d) => {
        d.groups = snap.groups ?? [];
        d.entries = snap.entries ?? [];
        d.loading = false;
      });
    }

    async function refresh() {
      const snap = await native.invoke<BookmarksSnapshot>('bookmarks.list');
      applySnapshot(snap);
    }

    return {
      async onLoad() {
        try {
          await refresh();
          const stream = native.stream<BookmarksSnapshot>('bookmarks.watch');
          unwatch = () => stream.close();
          (async () => {
            for await (const snap of stream) {
              if (disposed) break;
              applySnapshot(snap);
            }
          })().catch(() => {});
        } catch {
          if (!disposed) page.update((d) => { d.loading = false; });
        }
      },
      onUnload() {
        disposed = true;
        unwatch?.();
        unwatch = null;
      },
      actions: {
        selectGroup({ groupId }: { groupId: string }) {
          page.update((d) => { d.selectedGroupId = groupId; });
        },
        async addBookmark({ title, url, groupId }: { title: string; url: string; groupId?: string | null }) {
          const snap = await native.invoke<BookmarksSnapshot>('bookmarks.add', {
            url: url.trim(),
            title: title.trim() || undefined,
            groupId: groupId && !groupId.startsWith('__') ? groupId : undefined,
          });
          applySnapshot(snap);
        },
        async removeBookmark({ id }: { id: string }) {
          const snap = await native.invoke<BookmarksSnapshot>('bookmarks.remove', { id });
          applySnapshot(snap);
        },
        async togglePin({ id, pinned }: { id: string; pinned: boolean }) {
          await native.invoke('bookmarks.setPinned', { id, pinned });
          await refresh();
        },
        async createGroup({ name }: { name: string }) {
          const trimmed = name.trim();
          if (!trimmed) return;
          const snap = await native.invoke<BookmarksSnapshot>('bookmarks.createGroup', { name: trimmed });
          applySnapshot(snap);
        },
        async deleteGroup({ id }: { id: string }) {
          const snap = await native.invoke<BookmarksSnapshot>('bookmarks.deleteGroup', { id });
          applySnapshot(snap);
          page.update((d) => {
            if (d.selectedGroupId === id) d.selectedGroupId = '__all__';
          });
        },
        async importHtml() {
          const snap = await native.invoke<BookmarksSnapshot>('bookmarks.importHtml');
          applySnapshot(snap);
        },
        async exportHtml() {
          await native.invoke('bookmarks.exportHtml');
          page.update((d) => { d.statusMessage = 'Exported'; });
        },
        async openUrl({ url }: { url: string }) {
          await lx.surface.openUrl(url);
        },
      },
    };
  },
});
