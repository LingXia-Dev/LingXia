import { browserNative } from '../../shared/native';
import { definePage, lx } from '@lingxia/logic';

interface DownloadRecord {
  taskId: string;
  fileName: string;
  url: string;
  status: string;
  downloadedBytes: number;
  totalBytes?: number | null;
  errorMessage?: string | null;
  retry?: boolean;
}

interface DownloadsSnapshot {
  downloads: DownloadRecord[];
}

interface DownloadsData {
  downloads: DownloadRecord[];
  filter: 'all' | 'active' | 'completed';
  loading: boolean;
}

const initialData: DownloadsData = {
  downloads: [],
  filter: 'all',
  loading: true,
};

export default definePage({
  data: initialData,
  setup(page) {
    const native = browserNative(page);
    let disposed = false;
    let unwatch: (() => void) | null = null;

    function applySnapshot(snap: DownloadsSnapshot) {
      if (disposed) return;
      page.update((d) => {
        d.downloads = snap?.downloads ?? [];
        d.loading = false;
      });
    }

    async function refresh() {
      const snap = await native.invoke<DownloadsSnapshot>('downloads.list');
      applySnapshot(snap);
    }

    return {
      async onLoad() {
        try {
          await refresh();
          const stream = native.stream<DownloadsSnapshot>('downloads.watch');
          unwatch = () => stream.close();
          (async () => {
            for await (const snap of stream) {
              if (disposed) break;
              if (snap?.downloads) applySnapshot(snap);
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
        setFilter({ filter }: { filter: 'all' | 'active' | 'completed' }) {
          page.update((d) => { d.filter = filter; });
        },
        async pause({ taskId }: { taskId: string }) {
          await native.invoke('downloads.pause', { taskId });
          await refresh();
        },
        async resume({ taskId }: { taskId: string }) {
          await native.invoke('downloads.resume', { taskId });
          await refresh();
        },
        async retry({ taskId }: { taskId: string }) {
          await native.invoke('downloads.retry', { taskId });
          await refresh();
        },
        async cancel({ taskId }: { taskId: string }) {
          await native.invoke('downloads.cancel', { taskId });
          await refresh();
        },
        async remove({ taskId }: { taskId: string }) {
          await native.invoke('downloads.remove', { taskId });
          await refresh();
        },
        async open({ taskId }: { taskId: string }) {
          await native.invoke('downloads.open', { taskId });
        },
        async reveal({ taskId }: { taskId: string }) {
          await native.invoke('downloads.reveal', { taskId });
        },
        async clearCompleted() {
          await native.invoke('downloads.clearCompleted');
          await refresh();
        },
        async openSettings() {
          await lx.nav.go({ page: 'settings' });
        },
      },
    };
  },
});
