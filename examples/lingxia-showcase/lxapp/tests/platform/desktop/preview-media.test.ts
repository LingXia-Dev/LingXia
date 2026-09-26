import { expect, spec } from '@lingxia/test';
import type { DesktopWindowInfo } from '@lingxia/types/automation';
import { SHOWCASE_APP_ID } from '../../helpers/app.js';
import { runtimePlatform } from '../../helpers/platform.js';
import { bindFixture, eventually } from '../../helpers/poll.js';
import type { PreviewMediaHandle } from '@lingxia/types';

/** What the probes keep on Logic's `globalThis` under their spec's key. */
interface PreviewSession {
  handle: PreviewMediaHandle;
  controller?: AbortController;
  presented: boolean;
  changes?: number;
  completed: unknown;
  off?: (() => void) | null;
}
type Sessions = Record<string, PreviewSession | undefined>;

const testArgs = globalThis.__LINGXIA_AUTOMATION_HOST__?.args ?? {} as Record<string, string>;
const httpBase = testArgs.httpBase;
const selectedGate = testArgs.gate?.toLocaleLowerCase();
const previewSpec = httpBase && !selectedGate ? spec : spec.skip;
const httpsPreviewSpec = selectedGate ? spec.skip : spec;

/** Public samples the Showcase already loads; scheme is `https://` (not the HTTP fixture). */
const HTTPS_IMAGE =
  'https://cn.bing.com/th?id=OHR.BulgariaRocks_EN-US3184562282_UHD.jpg';
/** Answers 404 with an XML body, so no host can mistake it for an image. */
const HTTPS_MISSING_IMAGE =
  'https://interactive-examples.mdn.mozilla.net/media/cc0-images/lingxia-missing-404.png';

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
  const desktop = t.automation.desktop;
  const stateKey = `__lingxiaPreview_${namespace.replace(/-/g, '_')}`;
  const readState = () => app.logic.eval((_, key): HandleState => {
    const s = (globalThis as unknown as Sessions)[key];
    return {
      presented: !!s?.presented,
      currentIndex: s?.handle.current.index ?? -1,
      currentPath: s?.handle.current.source.path ?? '',
      changes: s?.changes ?? 0,
      completed: (s?.completed ?? null) as HandleState['completed'],
    };
  }, stateKey);

  // Best effort only: leaving the panel up would sit on the developer's screen,
  // but failing to take it down is not this case's contract.
  const appWindows = await t.automation.lxapps.windows();
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
    await app.logic.eval((_, key) => {
      const sessions = globalThis as unknown as Sessions;
      sessions[key]?.off?.();
      delete sessions[key];
    }, stateKey).catch(() => undefined);
  });

  const started = await app.logic.eval({ timeout: 20_000 }, async ({ lx }, key, base) => {
    const png = await lx.downloadFile({ url: base + '/media/sample.png' }).result;
    const handle = lx.previewMedia({ path: png.uri, type: 'image' });
    const state: PreviewSession = { handle, presented: false, changes: 0, completed: null, off: null };
    (globalThis as unknown as Sessions)[key] = state;
    state.off = handle.onChange(() => { state.changes = (state.changes ?? 0) + 1; });
    handle.presented.then((outcome) => { state.presented = outcome.status === 'presented'; });
    handle.completed.then((result) => { state.completed = { reason: result.reason, index: result.index }; });
    return { path: png.uri, index: handle.current.index, sourcePath: handle.current.source.path };
  }, stateKey, httpBase ?? '');
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
    const offTwice = await app.logic.eval((_, key) => {
      const s = (globalThis as unknown as Sessions)[key]!;
      s.off!();
      s.off!();
      return true;
    }, stateKey);
    expect(offTwice).toBe(true);
  });
});

httpsPreviewSpec('skip an unreachable https item and keep request indexes', {
  id: 'DESKTOP-PREVIEW-HTTPS-SKIP-001',
  covers: [
    'lx.previewMedia',
    'PreviewMediaHandle.current',
    'PreviewMediaHandle.onChange',
  ],
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
}, async (t) => {
  const { app, namespace, defer } = bindFixture(t, 'DESKTOP-PREVIEW-HTTPS-SKIP-001');
  // Windows reports no change stream at all, so `current` cannot move there.
  if (await runtimePlatform(app) !== 'macos') return;
  const stateKey = `__lingxiaPreviewSkip_${namespace.replace(/-/g, '_')}`;
  defer(async () => {
    await app.logic.eval((_, key) => {
      const sessions = globalThis as unknown as Sessions;
      const s = sessions[key];
      if (s?.controller && !s.controller.signal.aborted) s.controller.abort();
      delete sessions[key];
    }, stateKey).catch(() => undefined);
  });

  await app.logic.eval(({ lx }, key, missing, image) => {
    const controller = new AbortController();
    const handle = lx.previewMedia({
      sources: [
        { path: missing, type: 'image' },
        { path: image, type: 'image', durationMs: 60000 },
      ],
      startIndex: 0,
      advance: 'next',
      signal: controller.signal,
    });
    const state: PreviewSession = { handle, controller, presented: false, completed: null };
    (globalThis as unknown as Sessions)[key] = state;
    handle.presented.then((outcome) => { state.presented = outcome.status === 'presented'; });
    handle.completed.then(
      (result) => { state.completed = result.reason; },
      () => { state.completed = 'rejected'; },
    );
    return true;
  }, stateKey, HTTPS_MISSING_IMAGE, HTTPS_IMAGE);

  // The 404 is left out of what the panel shows, yet the item on screen must
  // still be reported by its index in the request, not the panel's own.
  const state = await eventually(
    () => app.logic.eval((_, key) => {
      const s = (globalThis as unknown as Sessions)[key];
      return {
        presented: !!s?.presented,
        index: s?.handle.current.index ?? -1,
        path: s?.handle.current.source.path ?? '',
        completed: (s?.completed ?? null) as string | null,
      };
    }, stateKey),
    (value) => value.presented && value.index === 1,
    { describe: 'the preview to open on the reachable item as index 1', timeoutMs: 30_000 },
  );
  expect(state.path).toBe(HTTPS_IMAGE);
  expect(state.completed).toBe(null);
});
