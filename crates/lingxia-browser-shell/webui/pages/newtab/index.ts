import { invoke, watch } from '../../shared/native';
import { definePage, lx } from '@lingxia/logic';

interface Shortcut {
  id: string;
  title: string;
  url: string;
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
    function applyBookmarks(view: { entries: Shortcut[] }) {
      page.update((d) => { d.shortcuts = view.entries.slice(0, 12); });
    }

    async function load() {
      const [info, closed] = await Promise.all([
        invoke<{ productName: string; version: string }>('app.getInfo').catch(() => null),
        invoke<ClosedTab[]>('tabs.recentlyClosed').catch(() => []),
      ]);
      page.update((d) => {
        if (info) {
          d.appName = info.productName;
          d.version = info.version;
        }
        d.recentlyClosed = closed.slice(0, 8);
        d.loading = false;
      });
    }

    return {
      async onLoad() {
        // The watch opens with the current bookmarks.
        watch<{ entries: Shortcut[] }>('bookmarks.watch', page.signal, applyBookmarks);
        await load();
      },
      actions: {
        async openUrl({ url }: { url: string }) {
          const trimmed = (url ?? '').trim();
          if (!trimmed) return;
          const target = /^[a-z][a-z0-9+.-]*:/i.test(trimmed) ? trimmed : `https://${trimmed}`;
          await lx.surface.openUrl(target);
        },
        async reopenTab({ id }: { id: string }) {
          await invoke('tabs.reopen', { id });
          await load();
        },
        async openPage({ page: name }: { page: 'bookmarks' | 'history' | 'downloads' | 'settings' }) {
          await lx.nav.go({ page: name });
        },
      },
    };
  },
});
