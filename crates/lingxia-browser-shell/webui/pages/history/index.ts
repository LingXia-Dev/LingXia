import { invoke, watch } from '../../shared/native';
import { definePage, lx } from '@lingxia/logic';

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

interface HistoryData {
  entries: HistoryEntry[];
  loading: boolean;
}

const initialData: HistoryData = {
  entries: [],
  loading: true,
};

export default definePage({
  data: initialData,
  setup(page) {
    async function refresh() {
      const snap = await invoke<{ entries: HistoryEntry[] }>('history.list');
      page.update((d) => {
        d.entries = snap.entries;
        d.loading = false;
      });
    }

    function apply(event: HistoryWatchEvent) {
      page.update((d) => {
        if (event.kind === 'snapshot') {
          d.entries = event.entries;
        } else if (event.kind === 'upsert') {
          const rest = d.entries.filter((entry) => entry.id !== event.entry.id);
          d.entries = [event.entry, ...rest].sort((a, b) => b.visitedAtMs - a.visitedAtMs);
        } else {
          d.entries = d.entries.filter((entry) => entry.id !== event.id);
        }
        d.loading = false;
      });
    }

    return {
      async onLoad() {
        watch<HistoryWatchEvent>('history.watch', page.signal, apply);
        await refresh().catch(() => page.update((d) => { d.loading = false; }));
      },
      actions: {
        async removeEntry({ id }: { id: string }) {
          await invoke('history.remove', { id });
          await refresh();
        },
        async clearRange({ sinceMs }: { sinceMs?: number }) {
          await invoke('history.clear', sinceMs !== undefined ? { sinceMs } : {});
          await refresh();
        },
        async openUrl({ url }: { url: string }) {
          await lx.surface.openUrl(url);
        },
      },
    };
  },
});
