import { expect, spec } from '@lingxia/test';
import type { DesktopWindowInfo } from 'lingxia-types/automation';
import { SHOWCASE_APP_ID } from '../../helpers/app.js';
import { runtimePlatform } from '../../helpers/platform.js';
import { bindFixture, eventually } from '../../helpers/poll.js';

const testArgs = globalThis.__LINGXIA_AUTOMATION_HOST__?.args ?? {} as Record<string, string>;
const httpBase = testArgs.httpBase;
const selectedGate = testArgs.gate?.toLocaleLowerCase();
const previewSpec = httpBase && !selectedGate ? spec : spec.skip;

interface HandleState {
  presented: boolean;
  currentIndex: number;
  currentPath: string;
  changes: number;
  completed: { reason: string; index: number } | null;
}

/**
 * On a desktop `previewMedia` presents the host's own preview window (Quick
 * Look on macOS), which is not modal: the handle keeps reporting while it is
 * up, and closing it through AX settles `completed` with reason `manual`.
 */
previewSpec('present a local image in the native preview and close it from the handle side', {
  id: 'DESKTOP-PREVIEW-MEDIA-001',
  covers: [
    'lx.previewMedia',
    'PreviewMediaHandle.presented',
    'PreviewMediaHandle.current',
    'PreviewMediaHandle.onChange',
    'PreviewMediaHandle.completed',
    'DesktopDriver.windows',
    'DesktopAx.query',
    'DesktopKey.press',
  ],
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
  reason: 'needs the HTTP fixture: node tests/harness/http-fixture.mjs, then --arg httpBase=<url>',
}, async (t) => {
  const { app, namespace, defer } = bindFixture(t, 'DESKTOP-PREVIEW-MEDIA-001');
  const platform = await runtimePlatform(app);
  if (!['macos', 'windows'].includes(platform)) {
    throw new Error(`native preview requires macOS or Windows; got ${platform || 'unknown'}`);
  }
  const desktop = lx.automation().desktop;
  const stateKey = `__lingxiaPreview_${namespace.replace(/-/g, '_')}`;
  const readState = () => app.eval({
    script: `
      const s = globalThis[${JSON.stringify(stateKey)}];
      return {
        presented: !!s?.presented,
        currentIndex: s?.handle.current.index ?? -1,
        currentPath: s?.handle.current.source.path ?? '',
        changes: s?.changes ?? 0,
        completed: s?.completed ?? null,
      };
    `,
  }) as Promise<HandleState>;
  const hostPid = (await desktop.windows()).find((window) => window.process.toLowerCase().startsWith('lingxia'))?.pid;
  if (!hostPid) throw new Error('host window not found on the desktop');
  const before = new Set((await desktop.windows()).filter((window) => window.pid === hostPid).map((window) => window.id));
  const previewWindow = async (): Promise<DesktopWindowInfo | undefined> => (await desktop.windows())
    .find((window) => window.pid === hostPid && window.visible && !before.has(window.id));
  const closeButton = async (window: string) => (await desktop.ax.query({ window, match: 'name:close button', all: true }))[0];
  defer(async () => {
    const stray = await previewWindow();
    if (stray) {
      const button = await closeButton(stray.id).catch(() => undefined);
      if (button) await desktop.ax.invoke({ window: stray.id, match: `id:${button.id}` }).catch(() => undefined);
    }
    await app.eval({ script: `const s = globalThis[${JSON.stringify(stateKey)}]; if (s?.off) s.off(); delete globalThis[${JSON.stringify(stateKey)}];` }).catch(() => undefined);
  });

  const started = await app.eval({
    timeoutMs: 20_000,
    script: `
      const png = await lx.downloadFile({ url: ${JSON.stringify(`${httpBase}/media/sample.png`)} });
      const handle = lx.previewMedia({ path: png.tempFilePath, type: 'image' });
      const state = { handle, presented: false, changes: 0, completed: null, off: null };
      globalThis[${JSON.stringify(stateKey)}] = state;
      state.off = handle.onChange(() => { state.changes += 1; });
      handle.presented.then(() => { state.presented = true; });
      handle.completed.then((result) => { state.completed = { reason: result.reason, index: result.index }; });
      return { path: png.tempFilePath, index: handle.current.index, sourcePath: handle.current.source.path };
    `,
  }) as { path: string; index: number; sourcePath: string };
  // `current` is synchronous and already describes the first source.
  expect(started.index).toBe(0);
  expect(started.sourcePath).toBe(started.path);

  await t.step('presented resolves once the preview window is up', async () => {
    await eventually(readState, (state) => state.presented, { describe: 'presented to resolve', timeoutMs: 15_000 });
    const window = await eventually(previewWindow, (found) => found !== undefined, {
      describe: 'the native preview window to appear',
      timeoutMs: 10_000,
    });
    expect(window).toBeDefined();
    expect((await readState()).completed).toBe(null);
  });

  await t.step('closing the preview settles completed with reason manual and no index change', async () => {
    const window = await previewWindow();
    if (!window) throw new Error('preview window disappeared before close');
    await eventually(() => closeButton(window.id), (found) => found !== undefined, {
      describe: 'the preview close button',
      timeoutMs: 10_000,
    });
    // Quick Look dismisses on Escape. The in-process AX driver cannot press
    // the host's own close button, so the key is the primary path and AX the
    // fallback; either way the window going away is what proves the close.
    await desktop.window.focus({ window: window.id }).catch(() => undefined);
    await desktop.key.press({ key: 'Escape' });
    const closed = await eventually(previewWindow, (found) => found === undefined, {
      describe: 'preview window to close after Escape',
      timeoutMs: 5_000,
    }).then(() => true).catch(() => false);
    if (!closed) {
      await desktop.ax.invoke({ window: window.id, match: 'name:close button' });
      await eventually(previewWindow, (found) => found === undefined, { describe: 'preview window to close', timeoutMs: 10_000 });
    }
    const state = await eventually(readState, (value) => value.completed !== null, {
      describe: 'completed to settle after the window closed',
      timeoutMs: 10_000,
    });
    expect(state.completed?.reason).toBe('manual');
    expect(state.completed?.index).toBe(0);
    expect(state.changes).toBe(0);
    await eventually(previewWindow, (found) => found === undefined, { describe: 'preview window to close', timeoutMs: 10_000 });
    // The listener handle is inert after unsubscribe, twice.
    const offTwice = await app.eval({
      script: `const s = globalThis[${JSON.stringify(stateKey)}]; s.off(); s.off(); return true;`,
    });
    expect(offTwice).toBe(true);
  });
});
