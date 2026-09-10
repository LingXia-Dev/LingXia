import { expect, spec, type Fixture } from '@lingxia/test';
import type { DesktopWindowInfo } from '@lingxia/types/automation';
import { SHOWCASE_APP_ID } from '../../helpers/app.js';
import { runtimePlatform } from '../../helpers/platform.js';
import { bindFixture, eventually } from '../../helpers/poll.js';

const testArgs = globalThis.__LINGXIA_AUTOMATION_HOST__?.args ?? {} as Record<string, string>;
const httpBase = testArgs.httpBase;
const selectedGate = testArgs.gate?.toLocaleLowerCase();
const previewSpec = httpBase && !selectedGate ? spec : spec.skip;
const httpsPreviewSpec = selectedGate ? spec.skip : spec;

/** Public samples the Showcase already loads; scheme is `https://` (not the HTTP fixture). */
const HTTPS_IMAGE =
  'https://cn.bing.com/th?id=OHR.BulgariaRocks_EN-US3184562282_UHD.jpg';
const HTTPS_VIDEO =
  'https://interactive-examples.mdn.mozilla.net/media/cc0-videos/flower.mp4';

interface HandleState {
  presented: boolean;
  currentIndex: number;
  currentPath: string;
  changes: number;
  completed: { reason: string; index: number } | null;
}

/**
 * `previewMedia` hands back a handle synchronously and presents the host's own
 * preview surface. What this case proves is that handle: the host says it
 * presented, `current` describes the source it was opened with, and the change
 * listener is a real subscription.
 *
 * It deliberately does not assert on desktop windows. The native panel is a
 * persistent singleton — it outlives the page that opened it and stays around
 * once created — so "a new window appeared" depends on whatever ran before,
 * and dismissing it needs a gesture no in-process driver can place reliably
 * on a desktop where another app may hold focus. PEND-PREVIEW-DISMISS-001
 * owns the dismissal contract.
 */
previewSpec('present a local image and report it through the handle', {
  id: 'DESKTOP-PREVIEW-MEDIA-001',
  covers: [
    'lx.previewMedia',
    'PreviewMediaHandle.presented',
    'PreviewMediaHandle.current',
    'PreviewMediaHandle.onChange',
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

  // Best effort only: leaving the panel up would sit on the developer's screen,
  // but failing to take it down is not this case's contract.
  const appWindows = await lx.automation().lxapps.windows();
  const hostWindowId = (appWindows.find((window) => window.main) ?? appWindows[0])?.id;
  const hostPid = (await desktop.windows()).find((window) => window.id === hostWindowId)?.pid;
  const before = new Set((await desktop.windows())
    .filter((window) => window.visible)
    .map((window) => window.id));
  const strayPanel = async (): Promise<DesktopWindowInfo | undefined> => (await desktop.windows())
    .find((window) => window.pid === hostPid && window.visible && !before.has(window.id));
  defer(async () => {
    const stray = await strayPanel().catch(() => undefined);
    if (stray) await desktop.window.close({ window: stray.id }).catch(() => undefined);
    await app.eval({
      script: `const s = globalThis[${JSON.stringify(stateKey)}]; if (s?.off) s.off(); delete globalThis[${JSON.stringify(stateKey)}];`,
    }).catch(() => undefined);
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

  await t.step('presented resolves and nothing advanced the sequence', async () => {
    const state = await eventually(readState, (value) => value.presented, {
      describe: 'the host to report the preview presented',
      timeoutMs: 15_000,
    });
    expect(state.currentIndex).toBe(0);
    expect(state.currentPath).toBe(started.path);
    // A single source with no advance emits no change and does not complete
    // on its own.
    expect(state.changes).toBe(0);
    expect(state.completed).toBe(null);
  });

  await t.step('the change subscription is a real handle', async () => {
    const offTwice = await app.eval({
      script: `const s = globalThis[${JSON.stringify(stateKey)}]; s.off(); s.off(); return true;`,
    });
    expect(offTwice).toBe(true);
  });
});

interface HttpsHandleState {
  presented: boolean;
  currentIndex: number;
  currentPath: string;
  completed: { reason: string; index: number } | null;
  completedError: { name: string; message: string } | null;
}

async function presentHttpsPreview(
  t: Fixture,
  id: string,
  url: string,
  type: 'image' | 'video',
): Promise<void> {
  const { app, namespace, defer } = bindFixture(t, id);
  const platform = await runtimePlatform(app);
  if (!['macos', 'windows'].includes(platform)) {
    throw new Error(`native preview requires macOS or Windows; got ${platform || 'unknown'}`);
  }
  const desktop = lx.automation().desktop;
  const stateKey = `__lingxiaPreviewHttps_${namespace.replace(/-/g, '_')}`;
  const readState = () => app.eval({
    script: `
      const s = globalThis[${JSON.stringify(stateKey)}];
      return {
        presented: !!s?.presented,
        currentIndex: s?.handle.current.index ?? -1,
        currentPath: s?.handle.current.source.path ?? '',
        completed: s?.completed ?? null,
        completedError: s?.completedError ?? null,
      };
    `,
  }) as Promise<HttpsHandleState>;

  const appWindows = await lx.automation().lxapps.windows();
  const hostWindowId = (appWindows.find((window) => window.main) ?? appWindows[0])?.id;
  const hostPid = (await desktop.windows()).find((window) => window.id === hostWindowId)?.pid;
  const before = new Set((await desktop.windows())
    .filter((window) => window.visible)
    .map((window) => window.id));
  const strayPanel = async (): Promise<DesktopWindowInfo | undefined> => (await desktop.windows())
    .find((window) => window.pid === hostPid && window.visible && !before.has(window.id));
  defer(async () => {
    await app.eval({
      script: `
        const s = globalThis[${JSON.stringify(stateKey)}];
        if (s?.controller && !s.controller.signal.aborted) s.controller.abort();
        delete globalThis[${JSON.stringify(stateKey)}];
      `,
    }).catch(() => undefined);
    const stray = await strayPanel().catch(() => undefined);
    if (stray) await desktop.window.close({ window: stray.id }).catch(() => undefined);
  });

  const started = await app.eval({
    timeoutMs: 20_000,
    script: `
      const url = ${JSON.stringify(url)};
      const controller = new AbortController();
      const handle = lx.previewMedia({ path: url, type: ${JSON.stringify(type)}, signal: controller.signal });
      const state = { handle, controller, presented: false, completed: null, completedError: null };
      globalThis[${JSON.stringify(stateKey)}] = state;
      handle.presented.then(() => { state.presented = true; });
      handle.completed.then(
        (result) => { state.completed = { reason: result.reason, index: result.index }; },
        (error) => { state.completedError = { name: error && error.name, message: String(error && error.message || error) }; }
      );
      return { index: handle.current.index, sourcePath: handle.current.source.path };
    `,
  }) as { index: number; sourcePath: string };
  expect(started.index).toBe(0);
  expect(started.sourcePath).toBe(url);

  const state = await eventually(readState, (value) => value.presented, {
    describe: `the host to present the https ${type}`,
    timeoutMs: 30_000,
  });
  expect(state.currentIndex).toBe(0);
  expect(state.currentPath).toBe(url);
  // `presented` also settles when `completed` does (JS fallback / error close).
  // An open session is the proof the https source actually showed.
  expect(state.completed).toBe(null);
  expect(state.completedError).toBe(null);

  await app.eval({
    script: `globalThis[${JSON.stringify(stateKey)}].controller.abort(); return true;`,
  });
  await eventually(
    readState,
    (value) => value.completedError != null || value.completed != null,
    {
      describe: 'the AbortSignal to close the https preview session',
      timeoutMs: 10_000,
    },
  );
}

httpsPreviewSpec('present an https image without downloading first', {
  id: 'DESKTOP-PREVIEW-HTTPS-IMAGE-001',
  covers: [
    'lx.previewMedia',
    'PreviewMediaHandle.presented',
    'PreviewMediaHandle.current',
  ],
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
}, async (t) => {
  await presentHttpsPreview(t, 'DESKTOP-PREVIEW-HTTPS-IMAGE-001', HTTPS_IMAGE, 'image');
});

httpsPreviewSpec('present an https video without downloading first', {
  id: 'DESKTOP-PREVIEW-HTTPS-VIDEO-001',
  covers: [
    'lx.previewMedia',
    'PreviewMediaHandle.presented',
    'PreviewMediaHandle.current',
  ],
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
}, async (t) => {
  await presentHttpsPreview(t, 'DESKTOP-PREVIEW-HTTPS-VIDEO-001', HTTPS_VIDEO, 'video');
});
