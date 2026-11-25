import { invoke, watch } from '../../shared/native';
import { hostOf, localDateTime, originOf } from '../../shared/format';
import { siteIconCache } from '../../shared/siteIcons';
import { definePage, lx } from '@lingxia/logic';
import type { ClearBrowsingDataInput, ClearBrowsingDataResult } from '../../shared/privacy';

interface HistoryEntry {
  id: string;
  url: string;
  title: string;
  visitedAtMs: number;
}

type HistoryWatchEvent =
  | { kind: 'snapshot'; entries: HistoryEntry[] }
  | { kind: 'upsert'; entry: HistoryEntry }
  | { kind: 'remove'; id: string };

/** A day heading, or one visit under it; `at` and `day` are local `YYYY-MM-DDTHH:mm`. */
type HistoryRow =
  | { kind: 'day'; key: string; day: string }
  | { kind: 'visit'; key: string; id: string; url: string; title: string; host: string; at: string; icon?: string };

interface HistoryData {
  rows: HistoryRow[];
  /** Visits that match the search; `rows` holds the first of them and grows as the list nears its end. */
  shown: number;
  hasMore: boolean;
  total: number;
  /** The present and its day and the day before, for headings and times; refreshed with the rows. */
  now: string;
  today: string;
  yesterday: string;
  query: string;
  loading: boolean;
}

const initialData: HistoryData = {
  rows: [],
  shown: 0,
  hasMore: false,
  total: 0,
  now: '',
  today: '',
  yesterday: '',
  query: '',
  loading: true,
};

const PAGE = 200;
const SEARCH_SETTLE_MS = 150;

export default definePage({
  data: initialData,
  setup(page) {
    // Newest first; the rows are rebuilt from it, so the View gets one ready list.
    let entries: HistoryEntry[] = [];
    // Rows go out a page at a time, so no update carries the whole history.
    let limit = PAGE;
    // Icons that arrive after their rows went out are patched into just those rows.
    const icons = siteIconCache((found) => {
      page.update((d) => {
        for (const row of d.rows) {
          if (row.kind !== 'visit' || row.icon) continue;
          const icon = found.get(originOf(row.url) ?? '');
          if (icon) row.icon = icon;
        }
      });
    });

    function wantIcons(rows: HistoryRow[]) {
      void icons.want(rows.flatMap((row) => (row.kind === 'visit' && !row.icon ? [row.url] : [])));
    }

    function build() {
      const q = page.data.query.trim().toLowerCase();
      const rows: HistoryRow[] = [];
      let day = '';
      let shown = 0;
      for (const entry of entries) {
        if (q && !entry.title.toLowerCase().includes(q) && !entry.url.toLowerCase().includes(q)) continue;
        shown += 1;
        if (shown > limit) continue;
        const at = localDateTime(entry.visitedAtMs);
        const entryDay = `${at.slice(0, 10)}T00:00`;
        if (entryDay !== day) {
          day = entryDay;
          rows.push({ kind: 'day', key: `d${day}`, day });
        }
        rows.push({ kind: 'visit', key: entry.id, id: entry.id, url: entry.url, title: entry.title, host: hostOf(entry.url), at, icon: icons.iconOf(entry.url) });
      }
      return { rows, shown };
    }

    function publish() {
      const { rows, shown } = build();
      page.update((d) => {
        d.rows = rows;
        d.shown = shown;
        d.hasMore = shown > limit;
        d.total = entries.length;
        const now = Date.now();
        d.now = localDateTime(now);
        d.today = `${d.now.slice(0, 10)}T00:00`;
        d.yesterday = `${localDateTime(now - 86_400_000).slice(0, 10)}T00:00`;
        d.loading = false;
      });
      wantIcons(rows);
    }

    // Type-ahead settles before the store is searched, so only the last query of a burst runs.
    let pendingSearch: ReturnType<typeof setTimeout> | undefined;

    function apply(event: HistoryWatchEvent) {
      if (event.kind === 'snapshot') {
        entries = event.entries;
      } else if (event.kind === 'upsert') {
        entries = [event.entry, ...entries.filter((entry) => entry.id !== event.entry.id)]
          .sort((a, b) => b.visitedAtMs - a.visitedAtMs);
      } else {
        entries = entries.filter((entry) => entry.id !== event.id);
      }
      publish();
    }

    return {
      async onLoad() {
        // System pages open in their own stack; the bar shows back only when a page is under them.
        void page.navigationBar.update({ homeButton: 'hidden' });
        // The watch opens with a snapshot of the store, then follows each change.
        watch<HistoryWatchEvent>('history.watch', page.signal, apply);
      },
      actions: {
        async search({ query }: { query: string }) {
          if (pendingSearch !== undefined) clearTimeout(pendingSearch);
          pendingSearch = setTimeout(() => {
            pendingSearch = undefined;
            page.update((d) => { d.query = query; });
            limit = PAGE;
            publish();
          }, SEARCH_SETTLE_MS);
        },
        async loadMore() {
          if (!page.data.hasMore) return;
          limit += PAGE;
          const { rows, shown } = build();
          // The new rows extend the old ones, so only they cross to the View.
          const added = rows.slice(page.data.rows.length);
          page.update((d) => {
            d.rows.push(...added);
            d.hasMore = shown > limit;
          });
          wantIcons(added);
        },
        // The watch reports both, so the rows follow from it.
        async removeEntry({ id }: { id: string }) {
          await invoke('history.remove', { id });
        },
        /** Answers the toast's text, translated here: the View's `t` works only while rendering. */
        async clearBrowsingData(input: ClearBrowsingDataInput): Promise<string> {
          const result = await invoke<ClearBrowsingDataResult>('privacy.clearBrowsingData', input);
          return lx.app.t('privacy.cleared', { count: result.historyRemoved });
        },
        async openUrl({ url }: { url: string }) {
          await lx.surface.openUrl(url);
        },
      },
    };
  },
});
