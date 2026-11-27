import { definePage, lx } from '@lingxia/logic';
import type { BookmarkEntry, BookmarkGroup, BookmarksView } from '@lingxia/native';
import { siteIconCache } from '../../shared/siteIcons';

interface BookmarksData {
  groups: BookmarkGroup[];
  entries: BookmarkEntry[];
  pinnedIds: string[];
  selectedGroupId: string;
  /** Cached site icons by origin; a site without one shows its monogram. */
  icons: Record<string, string>;
  loading: boolean;
}

/** A toast's text, translated here: the View's `t` works only while rendering. */
type Notice = { message: string };
/** Why a dialog's save failed, in the user's words; the dialog stays open with its input. */
type Problem = { problem: string } | null;

/** The pin limit travels as `data.max`; anything else is said plainly, never as the route's text. */
function problemOf(error: unknown): Problem {
  console.warn('bookmark save failed', error);
  const max = (error as { data?: { max?: unknown } } | null)?.data?.max;
  if (typeof max === 'number') return { problem: lx.app.t('bookmarks.problem.sidebarFull', { max }) };
  return { problem: lx.app.t('bookmarks.problem.failed') };
}

const WEB_URL = /^https?:\/\/[^\s/?#]+/i;

const initialData: BookmarksData = {
  groups: [],
  entries: [],
  pinnedIds: [],
  selectedGroupId: '__all__',
  icons: {},
  loading: true,
};

export default definePage({
  data: initialData,
  setup(page) {
    const icons = siteIconCache(page.native.shell, (found) => {
      page.update((d) => {
        for (const [origin, icon] of found) d.icons[origin] = icon;
      });
    });

    function apply(view: BookmarksView) {
      void icons.want(view.entries.map((entry) => entry.url));
      page.update((d) => {
        d.groups = view.groups;
        d.entries = view.entries;
        d.pinnedIds = view.pinnedIds;
        // A scope whose rail entry went away falls back to All.
        const scope = d.selectedGroupId;
        const gone = scope === '__pinned__' ? !view.pinnedIds.length
          : scope === '__ungrouped__' ? !view.groups.length
            : scope !== '__all__' && !view.groups.some((group) => group.id === scope);
        if (gone) d.selectedGroupId = '__all__';
        d.loading = false;
      });
    }

    /** Creates a group, or renames group `id`; a name another group has is refused here, in words. */
    async function saveGroupName(name: string, id: string | undefined): Promise<Problem> {
      const trimmed = name.trim();
      if (!trimmed) return null;
      if (page.data.groups.some((group) => group.name === trimmed && group.id !== id)) {
        return { problem: lx.app.t('bookmarks.problem.groupExists', { name: trimmed }) };
      }
      try {
        if (id) await page.native.bookmarks.renameGroup({ id, name: trimmed });
        else await page.native.bookmarks.createGroup({ name: trimmed });
      } catch (error) {
        return problemOf(error);
      }
      await refresh();
      return null;
    }

    async function refresh() {
      apply(await page.native.bookmarks.list());
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
        page.native.bookmarks.watch(apply, { signal: page.signal });
        await refresh().catch(() => page.update((d) => { d.loading = false; }));
      },
      actions: {
        async selectGroup({ groupId }: { groupId: string }) {
          page.update((d) => { d.selectedGroupId = groupId; });
        },
        async addBookmark({ title, url, groupId }: { title: string; url: string; groupId: string }): Promise<Problem> {
          if (!WEB_URL.test(url.trim())) return { problem: lx.app.t('bookmarks.problem.url') };
          try {
            const added = await page.native.bookmarks.add({
              url: url.trim(),
              title: title.trim() || undefined,
              groupId: groupId.startsWith('__') ? undefined : groupId,
            });
            // Added while Pinned shows, it is pinned so it stays in view.
            if (groupId === '__pinned__') await page.native.bookmarks.setPinned({ id: added.entry.id, pinned: true });
          } catch (error) {
            return problemOf(error);
          }
          await refresh();
          return null;
        },
        /** Applies the edit form: only what changed goes to the store. */
        async updateBookmark({ id, title, groupId, pinned }: { id: string; title: string; groupId: string; pinned: boolean }): Promise<Problem> {
          const entry = page.data.entries.find((item) => item.id === id);
          if (!entry) return { problem: lx.app.t('bookmarks.problem.gone') };
          const nextGroup = groupId.startsWith('__') ? undefined : groupId;
          try {
            if (title.trim() !== entry.title) await page.native.bookmarks.rename({ id, title: title.trim() });
            if (nextGroup !== entry.groupId) await page.native.bookmarks.move({ id, groupId: nextGroup });
            if (pinned !== page.data.pinnedIds.includes(id)) await page.native.bookmarks.setPinned({ id, pinned });
          } catch (error) {
            await refresh();
            return problemOf(error);
          }
          await refresh();
          return null;
        },
        async removeBookmark({ id }: { id: string }) {
          await page.native.bookmarks.remove({ id });
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
          await page.native.bookmarks.reorder({ groupId: entry.groupId, orderedIds });
          await refresh();
        },
        async setPinned({ id, pinned }: { id: string; pinned: boolean }) {
          await page.native.bookmarks.setPinned({ id, pinned });
          await refresh();
        },
        async createGroup({ name }: { name: string }): Promise<Problem> {
          return saveGroupName(name, undefined);
        },
        async renameGroup({ id, name }: { id: string; name: string }): Promise<Problem> {
          if (!page.data.groups.some((group) => group.id === id)) return { problem: lx.app.t('bookmarks.problem.gone') };
          return saveGroupName(name, id);
        },
        async shiftGroup({ id, delta }: { id: string; delta: -1 | 1 }) {
          const orderedIds = shifted(page.data.groups.map((group) => group.id), id, delta);
          if (!orderedIds) return;
          await page.native.bookmarks.reorderGroups({ orderedIds });
          await refresh();
        },
        async deleteGroup({ id }: { id: string }) {
          await page.native.bookmarks.deleteGroup({ id });
          await refresh();
        },
        async importHtml(): Promise<Notice | null> {
          const result = await page.native.bookmarks.importHtml();
          if (!result) return null;
          await refresh();
          return { message: lx.app.t('bookmarks.status.imported', { count: result.imported }) };
        },
        async exportHtml(): Promise<Notice> {
          const { count } = await page.native.bookmarks.exportHtml();
          return { message: lx.app.t('bookmarks.status.exported', { count }) };
        },
        async openUrl({ url }: { url: string }) {
          await lx.surface.openUrl(url);
        },
      },
    };
  },
});
