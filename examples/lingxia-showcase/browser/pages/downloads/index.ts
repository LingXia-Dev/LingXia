import { localDateTime } from '../../shared/format';
import { definePage, lx } from '@lingxia/logic';
import type { DownloadRecord, DownloadsWatchEvent } from '@lingxia/native';

/** A download as the View shows it: when it last happened, in local form, and how fast it runs. */
type DownloadView = Pick<DownloadRecord, 'taskId' | 'url' | 'fileName' | 'status' | 'downloadedBytes' | 'totalBytes' | 'retry'> & {
  at: string;
  /** Why a failed download failed, in the user's language; the raw error stays in the log. */
  failure?: string;
  /** Bytes per second while downloading, else 0. */
  speed: number;
};

interface DownloadsData {
  downloads: DownloadView[];
  loading: boolean;
  /** The list could not be read. */
  loadFailed: boolean;
  /** Day starts for the Today and Yesterday groups, and the present. */
  now: string;
  today: string;
  yesterday: string;
}

const initialData: DownloadsData = {
  downloads: [],
  loading: true,
  loadFailed: false,
  now: '',
  today: '',
  yesterday: '',
};

// A speed sample needs this much time behind it to mean anything.
const SPEED_WINDOW_MS = 400;

function failureOf(error: string): string {
  const status = /^http status (\d+)/.exec(error);
  if (status) return lx.app.t('downloads.why.status', { code: status[1] });
  if (/proxy is disconnected/i.test(error)) return lx.app.t('downloads.why.proxy');
  if (/time(d)? ?out/i.test(error)) return lx.app.t('downloads.why.timeout');
  if (/permission denied|no space|read-only|disk full/i.test(error)) return lx.app.t('downloads.why.disk');
  return lx.app.t('downloads.why.network');
}

export default definePage({
  data: initialData,
  setup(page) {
    const samples = new Map<string, { bytes: number; time: number; speed: number }>();

    function speedOf(record: DownloadRecord): number {
      if (record.status !== 'downloading') {
        samples.delete(record.taskId);
        return 0;
      }
      const now = Date.now();
      const sample = samples.get(record.taskId);
      if (!sample) {
        samples.set(record.taskId, { bytes: record.downloadedBytes, time: now, speed: 0 });
        return 0;
      }
      const elapsed = now - sample.time;
      if (elapsed > SPEED_WINDOW_MS) {
        const gained = record.downloadedBytes - sample.bytes;
        sample.speed = gained > 0 ? (gained / elapsed) * 1000 : sample.speed / 2;
        sample.bytes = record.downloadedBytes;
        sample.time = now;
      }
      return sample.speed;
    }

    function viewOf(record: DownloadRecord): DownloadView {
      const { taskId, url, fileName, status, downloadedBytes, totalBytes, retry, error } = record;
      const failure = status === 'failed' ? failureOf(error ?? '') : undefined;
      if (error && status === 'failed') console.warn(`download ${taskId} failed: ${error}`);
      return {
        taskId, url, fileName, status, downloadedBytes, totalBytes, retry,
        at: localDateTime(record.completedAt ?? record.updatedAt),
        speed: speedOf(record),
        failure,
      };
    }

    function stampDays(d: DownloadsData) {
      const now = Date.now();
      d.now = localDateTime(now);
      d.today = `${d.now.slice(0, 10)}T00:00`;
      d.yesterday = `${localDateTime(now - 86_400_000).slice(0, 10)}T00:00`;
    }

    async function refresh() {
      try {
        const snap = await page.native.downloads.list();
        page.update((d) => {
          d.downloads = snap.downloads.map(viewOf);
          d.loading = false;
          d.loadFailed = false;
          stampDays(d);
        });
      } catch {
        page.update((d) => {
          d.loading = false;
          d.loadFailed = true;
        });
      }
    }

    function apply(watched: DownloadsWatchEvent) {
      if (watched.kind === 'snapshot') {
        page.update((d) => {
          d.downloads = watched.downloads.map(viewOf);
          d.loading = false;
          d.loadFailed = false;
          stampDays(d);
        });
        return;
      }
      const { event } = watched;
      const record = event.download;
      // Removals and clears carry no record; the list is the authority for them.
      if (!record || event.kind === 'removed' || event.kind === 'cleared') {
        void refresh();
        return;
      }
      page.update((d) => {
        const index = d.downloads.findIndex((item) => item.taskId === record.taskId);
        const shown = viewOf(record);
        if (index >= 0) d.downloads[index] = shown;
        else d.downloads = [shown, ...d.downloads];
      });
    }

    async function run(call: (input: { taskId: string }) => Promise<void>, taskId: string) {
      await call({ taskId });
      await refresh();
    }

    return {
      async onLoad() {
        // System pages open in their own stack; the bar shows back only when a page is under them.
        void page.navigationBar.update({ homeButton: 'hidden' });
        // The watch opens with the list, then follows each change; the read shows a failure.
        page.native.downloads.watch(apply, { signal: page.signal });
        await refresh();
      },
      actions: {
        async reload() {
          await refresh();
        },
        async pause({ taskId }: { taskId: string }) {
          await run(page.native.downloads.pause, taskId);
        },
        async resume({ taskId }: { taskId: string }) {
          await run(page.native.downloads.resume, taskId);
        },
        async retry({ taskId }: { taskId: string }) {
          await run(page.native.downloads.retry, taskId);
        },
        async cancel({ taskId }: { taskId: string }) {
          await run(page.native.downloads.cancel, taskId);
        },
        async remove({ taskId }: { taskId: string }) {
          await run(page.native.downloads.remove, taskId);
        },
        async open({ taskId }: { taskId: string }) {
          await page.native.downloads.open({ taskId });
        },
        async reveal({ taskId }: { taskId: string }) {
          await page.native.downloads.reveal({ taskId });
        },
        async clearCompleted() {
          await page.native.downloads.clearCompleted();
          await refresh();
        },
        async openSettings() {
          await lx.nav.go({ page: 'settings', params: { section: 'downloads' } });
        },
      },
    };
  },
});
