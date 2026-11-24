import { invoke, watch } from '../../shared/native';
import { definePage, lx } from '@lingxia/logic';

type DownloadStatus = 'downloading' | 'paused' | 'completed' | 'failed' | 'removed';

interface DownloadRecord {
  taskId: string;
  url: string;
  fileName: string;
  status: DownloadStatus;
  downloadedBytes: number;
  totalBytes?: number;
  retry: boolean;
  error?: string;
}

interface DownloadEvent {
  kind: 'started' | 'progress' | 'paused' | 'completed' | 'failed' | 'removed' | 'cleared';
  download?: DownloadRecord;
  taskId?: string;
}

interface DownloadsData {
  downloads: DownloadRecord[];
  loading: boolean;
}

const initialData: DownloadsData = {
  downloads: [],
  loading: true,
};

export default definePage({
  data: initialData,
  setup(page) {
    async function refresh() {
      const snap = await invoke<{ downloads: DownloadRecord[] }>('downloads.list');
      page.update((d) => {
        d.downloads = snap.downloads;
        d.loading = false;
      });
    }

    function apply(event: DownloadEvent) {
      const record = event.download;
      // Removals and clears carry no record; the list is the authority for them.
      if (!record || event.kind === 'removed' || event.kind === 'cleared') {
        void refresh().catch(() => {});
        return;
      }
      page.update((d) => {
        const index = d.downloads.findIndex((item) => item.taskId === record.taskId);
        if (index >= 0) d.downloads[index] = record;
        else d.downloads = [record, ...d.downloads];
      });
    }

    async function run(route: string, taskId: string) {
      await invoke(route, { taskId });
      await refresh();
    }

    return {
      async onLoad() {
        // System pages open in their own stack; the bar shows back only when a page is under them.
        void page.navigationBar.update({ homeButton: 'hidden' });
        watch<DownloadEvent>('downloads.watch', page.signal, apply);
        await refresh().catch(() => page.update((d) => { d.loading = false; }));
      },
      actions: {
        async pause({ taskId }: { taskId: string }) {
          await run('downloads.pause', taskId);
        },
        async resume({ taskId }: { taskId: string }) {
          await run('downloads.resume', taskId);
        },
        async retry({ taskId }: { taskId: string }) {
          await run('downloads.retry', taskId);
        },
        async cancel({ taskId }: { taskId: string }) {
          await run('downloads.cancel', taskId);
        },
        async remove({ taskId }: { taskId: string }) {
          await run('downloads.remove', taskId);
        },
        async open({ taskId }: { taskId: string }) {
          await invoke('downloads.open', { taskId });
        },
        async reveal({ taskId }: { taskId: string }) {
          await invoke('downloads.reveal', { taskId });
        },
        async clearCompleted() {
          await invoke('downloads.clearCompleted');
          await refresh();
        },
        async openSettings() {
          await lx.nav.go({ page: 'settings', params: { section: 'downloads' } });
        },
      },
    };
  },
});
