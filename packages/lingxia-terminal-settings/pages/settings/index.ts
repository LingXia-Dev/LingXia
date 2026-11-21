import { definePage, lx, type DownloadTask, type TransferProgress } from '@lingxia/logic';
type TerminalApi = NonNullable<typeof lx.host.terminal>;
type TerminalSnapshot = Awaited<ReturnType<TerminalApi['settings']['get']>>;
type WindowsTerminalApi = NonNullable<TerminalApi['windows']>;
type WindowsInlineImageStatus = Awaited<ReturnType<WindowsTerminalApi['status']>>;

type UpdateInput = {
  patch: Parameters<TerminalApi['settings']['update']>[0];
  ifRevision: number;
};

type ResetInput = {
  scope?: 'font' | 'theme';
  ifRevision: number;
};

type ImportInput = {
  text: string;
  name?: string;
  overwrite?: boolean;
};

type PreviewInput = {
  scheme: Parameters<
    ReturnType<TerminalApi['colorSchemes']['createPreview']>['show']
  >[0];
};

type ActionError = {
  code?: string;
  message: string;
};

type ActionResult<T> =
  | { ok: true; value: T }
  | { ok: false; error: ActionError };

function actionError(error: unknown): ActionError {
  const value = error as { code?: unknown; message?: unknown } | null;
  return {
    code: typeof value?.code === 'string' ? value.code : undefined,
    message: typeof value?.message === 'string' ? value.message : String(error),
  };
}

async function action<T>(operation: () => Promise<T>): Promise<ActionResult<T>> {
  try {
    return { ok: true, value: await operation() };
  } catch (error) {
    return { ok: false, error: actionError(error) };
  }
}

function terminal() {
  if (!lx.supports('host.terminal')) {
    throw new Error('Terminal settings are unavailable in this host');
  }
  return lx.host.terminal;
}

function windowsTerminal(): WindowsTerminalApi {
  const api = terminal().windows;
  if (!api) throw new Error('Windows terminal compatibility settings are unavailable');
  return api;
}

let preview: ReturnType<TerminalApi['colorSchemes']['createPreview']> | null = null;
let stopSettingsChanges: (() => void) | null = null;
let conptyDownload: DownloadTask | null = null;

function previewController(): ReturnType<TerminalApi['colorSchemes']['createPreview']> {
  preview ??= terminal().colorSchemes.createPreview();
  return preview;
}

export default definePage({
  data: {
    terminalSettingsSnapshot: null as TerminalSnapshot | null,
    windowsInlineImageProgress: null as TransferProgress | null,
  },
  setup(page) {
    const actions = {
      loadTerminalSettings() {
        return action(async () => {
          const api = terminal();
          const isWindows = lx.host.info.os === 'windows';
          const [snapshot, themes, fonts, windowsInlineImages] = await Promise.all([
            api.settings.get(),
            api.colorSchemes.list(),
            api.fonts.list(),
            isWindows ? windowsTerminal().status() : Promise.resolve(null),
          ]);
          return { snapshot, themes, fonts, isWindows, windowsInlineImages };
        });
      },
      setWindowsInlineImages(input: { enabled: boolean }) {
        return action(async (): Promise<WindowsInlineImageStatus> => {
          const api = windowsTerminal();
          if (!input.enabled) {
            await conptyDownload?.cancel();
            conptyDownload = null;
            page.update(d => { d.windowsInlineImageProgress = null; });
            return api.setEnabled({ enabled: false });
          }
          const current = await api.status();
          if (current.installed) return api.setEnabled({ enabled: true });

          const task = lx.network.download(current.package.url);
          conptyDownload = task;
          try {
            for await (const progress of task) {
              page.update(d => { d.windowsInlineImageProgress = progress; });
            }
            const result = await task.result;
            return await api.install({ path: result.path });
          } finally {
            if (conptyDownload === task) conptyDownload = null;
            page.update(d => { d.windowsInlineImageProgress = null; });
          }
        });
      },
      updateTerminalSettings(input: UpdateInput) {
        return action(() =>
          terminal().settings.update({ ...input.patch, ifRevision: input.ifRevision }),
        );
      },
      resetTerminalSettings(input: ResetInput) {
        return action(() => terminal().settings.reset(input));
      },
      importTerminalScheme(input: ImportInput) {
        return action(() => terminal().colorSchemes.import(input));
      },
      previewTerminalScheme(input: PreviewInput) {
        return action(() => previewController().show(input.scheme));
      },
      clearTerminalPreview() {
        return action(() => preview?.clear() ?? Promise.resolve());
      },
    };
    return {
      onLoad() {
        stopSettingsChanges?.();
        stopSettingsChanges = terminal().settings.onChange((snapshot) => {
          page.update(d => { d.terminalSettingsSnapshot = snapshot; });
        });
      },
      async onUnload() {
        stopSettingsChanges?.();
        stopSettingsChanges = null;
        const controller = preview;
        preview = null;
        await controller?.close();
        await conptyDownload?.cancel();
        conptyDownload = null;
      },
      actions,
    };
  },
});
