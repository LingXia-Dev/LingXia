import {
  definePage,
  isLxError,
  lx,
  type ConptyStatus,
  type DownloadTask,
  type TerminalFontSettings,
  type TerminalSettingsPatch,
  type TerminalSettingsSnapshot,
  type TransferProgress,
} from '@lingxia/logic';
import { mergeSchemes, type SchemeEntry } from './schemes.ts';

type Status = 'loading' | 'ready' | 'unavailable';

interface SettingsData {
  status: Status;
  snapshot: TerminalSettingsSnapshot | null;
  schemes: SchemeEntry[];
  /** Installed monospaced families, plus the one in use, sorted. */
  fonts: string[];
  /** Null off Windows, where there is no console runtime to choose. */
  inlineImages: InlineImages | null;
  inlineImagesProgress: TransferProgress | null;
}

/** The console runtime's state as the page shows it; its package URL stays in Logic. */
interface InlineImages {
  enabled: boolean;
  installed: boolean;
}

function inlineImagesOf({ enabled, installed }: ConptyStatus): InlineImages {
  return { enabled, installed };
}

export type ImportOutcome =
  | { status: 'imported'; name: string }
  | { status: 'canceled' };

const initialData: SettingsData = {
  status: 'loading',
  snapshot: null,
  schemes: [],
  fonts: [],
  inlineImages: null,
  inlineImagesProgress: null,
};

function terminal() {
  return lx.host.terminal;
}

function isRevisionOutdated(error: unknown): boolean {
  return isLxError(error) && error.code === 'E_INVALID_STATE' && error.reason === 'revisionOutdated';
}

function fontFamilies(fonts: readonly { family: string; monospace: boolean }[], current: string): string[] {
  const families = new Set(fonts.filter((font) => font.monospace).map((font) => font.family));
  if (current) families.add(current);
  return [...families].sort((a, b) => a.localeCompare(b));
}

export default definePage({
  data: initialData,
  setup(page) {
    let download: DownloadTask | null = null;
    // Writes run one at a time against the latest revision, so quick changes never race each other.
    let queue: Promise<unknown> = Promise.resolve();
    function serial<T>(work: () => Promise<T>): Promise<T> {
      const next = queue.then(work, work);
      queue = next.catch(() => {});
      return next;
    }

    function show(snapshot: TerminalSettingsSnapshot) {
      page.update((d) => { d.snapshot = snapshot; });
    }

    /** A field-level write; another writer moving the revision only means trying once more on top of it. */
    async function write(attempt: (revision: number) => Promise<TerminalSettingsSnapshot>): Promise<void> {
      return serial(async () => {
        const revision = page.data.snapshot?.revision ?? (await terminal().settings.get()).revision;
        try {
          show(await attempt(revision));
        } catch (error) {
          if (!isRevisionOutdated(error)) throw error;
          show(await attempt((await terminal().settings.get()).revision));
        }
      });
    }

    function patch(value: TerminalSettingsPatch) {
      return write((ifRevision) => terminal().settings.update({ ...value, ifRevision }));
    }

    async function refreshSchemes() {
      const schemes = mergeSchemes(await terminal().colorSchemes.list());
      page.update((d) => { d.schemes = schemes; });
    }

    return {
      async onLoad() {
        // A single-page app opened on its own; the bar has nothing to go home to.
        void page.navigationBar.update({ homeButton: 'hidden' });
        if (!lx.supports('host.terminal')) {
          page.update((d) => { d.status = 'unavailable'; });
          return;
        }
        const api = terminal();
        api.settings.onChange(show, { signal: page.signal });
        try {
          const onWindows = lx.supports('host.terminal.conpty');
          const [snapshot, schemes, fonts, inlineImages] = await Promise.all([
            api.settings.get(),
            api.colorSchemes.list(),
            api.fonts.list(),
            onWindows ? api.conpty.status() : Promise.resolve(null),
          ]);
          page.update((d) => {
            d.snapshot = snapshot;
            d.schemes = mergeSchemes(schemes);
            d.fonts = fontFamilies(fonts, snapshot.effective.font.family);
            d.inlineImages = inlineImages && inlineImagesOf(inlineImages);
            d.status = 'ready';
          });
        } catch (error) {
          console.warn('terminal settings failed to load', error);
          page.update((d) => { d.status = 'unavailable'; });
        }
      },
      async onUnload() {
        await download?.cancel();
        download = null;
      },
      actions: {
        /** Uses `name` for the appearance the product is in now; a packaged scheme is imported first. */
        /** Sets the scheme for one appearance slot; the other slot keeps its own. */
        async setScheme({ name, slot }: { name: string; slot: 'light' | 'dark' }): Promise<void> {
          const entry = page.data.schemes.find((scheme) => scheme.name === name);
          if (!entry) throw new Error(`no color scheme named ${name}`);
          if (!entry.installed) {
            await terminal().colorSchemes.import({ text: JSON.stringify(entry.scheme), name, overwrite: true });
            page.update((d) => {
              const listed = d.schemes.find((scheme) => scheme.name === name);
              if (listed) listed.installed = true;
            });
          }
          await write((ifRevision) => terminal().settings.update({ theme: { [slot]: name }, ifRevision }));
        },
        async setFont(font: Partial<Omit<TerminalFontSettings, 'family'>> & { family?: string }): Promise<void> {
          const { family, ...rest } = font;
          await patch({ font: family === undefined ? rest : { ...rest, family: [family] } });
        },
        async resetAll(): Promise<void> {
          await write((ifRevision) => terminal().settings.reset({ ifRevision }));
        },
        /** Picks a scheme file and imports it; the new scheme joins the list, chosen or not. */
        async importScheme(): Promise<ImportOutcome> {
          const picked = await terminal().colorSchemes.importFile();
          if (picked.status === 'canceled') return { status: 'canceled' };
          await refreshSchemes();
          return { status: 'imported', name: picked.scheme.scheme.name || picked.scheme.name };
        },
        /** Turning it on downloads and installs Microsoft's runtime first, if it is not installed yet. */
        async setInlineImages({ enabled }: { enabled: boolean }): Promise<void> {
          const api = terminal().conpty;
          if (!enabled) {
            await download?.cancel();
            download = null;
            const status = await api.setEnabled({ enabled: false });
            page.update((d) => { d.inlineImages = inlineImagesOf(status); d.inlineImagesProgress = null; });
            return;
          }
          const current = await api.status();
          if (current.installed) {
            const status = await api.setEnabled({ enabled: true });
            page.update((d) => { d.inlineImages = inlineImagesOf(status); });
            return;
          }
          const task = lx.network.download(String(current.package.url));
          download = task;
          try {
            for await (const progress of task) {
              page.update((d) => { d.inlineImagesProgress = progress; });
            }
            const status = await api.install({ path: (await task.result).path });
            page.update((d) => { d.inlineImages = inlineImagesOf(status); });
          } finally {
            if (download === task) download = null;
            page.update((d) => { d.inlineImagesProgress = null; });
          }
        },
      },
    };
  },
});
