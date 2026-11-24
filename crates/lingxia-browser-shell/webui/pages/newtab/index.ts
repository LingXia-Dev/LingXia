import { browserNative } from '../../shared/native';
import { definePage, lx } from '@lingxia/logic';

interface Shortcut {
  id: string;
  title: string;
  url: string;
  pinned?: boolean;
}

interface ClosedTab {
  id: string;
  title: string;
  url: string;
}

interface NewTabData {
  appName: string;
  version: string;
  shortcuts: Shortcut[];
  recentlyClosed: ClosedTab[];
  loading: boolean;
}

const initialData: NewTabData = {
  appName: 'LingXia',
  version: '',
  shortcuts: [],
  recentlyClosed: [],
  loading: true,
};

export default definePage({
  data: initialData,
  setup(page) {
    const native = browserNative(page);
    let disposed = false;
    let unwatch: (() => void) | null = null;

    async function load() {
      try {
        const [info, bm, closed] = await Promise.all([
          native.invoke<{ appName?: string; version?: string }>('app.getInfo').catch(() => null),
          native.invoke<{ entries?: Shortcut[] }>('bookmarks.list').catch(() => ({ entries: [] })),
          native.invoke<{ tabs?: ClosedTab[] }>('tabs.recentlyClosed').catch(() => ({ tabs: [] })),
        ]);
        if (disposed) return;
        page.update((d) => {
          if (info?.appName) d.appName = info.appName;
          if (info?.version) d.version = info.version;
          d.shortcuts = (bm?.entries ?? []).slice(0, 12);
          d.recentlyClosed = (closed?.tabs ?? []).slice(0, 8);
          d.loading = false;
        });
      } catch {
        if (!disposed) page.update((d) => { d.loading = false; });
      }
    }

    return {
      async onLoad() {
        await load();
        try {
          const stream = native.stream<{ entries?: Shortcut[] }>('bookmarks.watch');
          unwatch = () => stream.close();
          (async () => {
            for await (const snap of stream) {
              if (disposed) break;
              if (snap?.entries) {
                page.update((d) => { d.shortcuts = snap.entries!.slice(0, 12); });
              }
            }
          })().catch(() => {});
        } catch {}
      },
      onUnload() {
        disposed = true;
        unwatch?.();
        unwatch = null;
      },
      actions: {
        async openUrl({ url }: { url: string }) {
          const trimmed = (url ?? '').trim();
          if (!trimmed) return;
          const target = /^https?:\/\//i.test(trimmed) ? trimmed : `https://${trimmed}`;
          await lx.surface.openUrl(target);
        },
        async reopenTab({ id }: { id: string }) {
          if (!id) return;
          await native.invoke('tabs.reopen', { id });
          await load();
        },
        async openPage({ page: name }: { page: 'bookmarks' | 'history' | 'downloads' | 'settings' }) {
          await lx.nav.go({ page: name });
        },
      },
    };
  },
});
