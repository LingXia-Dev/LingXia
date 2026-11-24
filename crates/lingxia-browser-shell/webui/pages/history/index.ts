import { browserNative } from '../../shared/native';
import { definePage, lx } from '@lingxia/logic';

interface HistoryEntry {
  id: string;
  url: string;
  title: string;
  visitedAtMs: number;
}

interface HistorySnapshot {
  entries: HistoryEntry[];
}

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
    const native = browserNative(page);
    let disposed = false;
    let unwatch: (() => void) | null = null;

    function applySnapshot(snap: HistorySnapshot) {
      if (disposed) return;
      page.update((d) => {
        d.entries = snap.entries ?? [];
        d.loading = false;
      });
    }

    return {
      async onLoad() {
        try {
          const snap = await native.invoke<HistorySnapshot>('history.list');
          applySnapshot(snap);
          const stream = native.stream<HistorySnapshot>('history.watch');
          unwatch = () => stream.close();
          (async () => {
            for await (const next of stream) {
              if (disposed) break;
              applySnapshot(next);
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
        async removeEntry({ id }: { id: string }) {
          const snap = await native.invoke<HistorySnapshot>('history.remove', { id });
          applySnapshot(snap);
        },
        async clearRange({ sinceMs }: { sinceMs?: number }) {
          const snap = await native.invoke<HistorySnapshot>(
            'history.clear',
            sinceMs !== undefined ? { sinceMs } : {},
          );
          applySnapshot(snap);
        },
        async openUrl({ url }: { url: string }) {
          await lx.surface.openUrl(url);
        },
      },
    };
  },
});
