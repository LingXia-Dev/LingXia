import { expect, spec, type Fixture } from '@lingxia/test';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { bindFixture, eventually } from '../helpers/poll.js';

const testArgs = globalThis.__LINGXIA_AUTOMATION_HOST__?.args ?? {} as Record<string, string>;
const selectedGate = testArgs.gate?.toLocaleLowerCase();
const httpsPreviewSpec = selectedGate ? spec.skip : spec;

/** Public samples the Showcase already loads; scheme is `https://` (not the HTTP fixture). */
const HTTPS_IMAGE =
  'https://cn.bing.com/th?id=OHR.BulgariaRocks_EN-US3184562282_UHD.jpg';
const HTTPS_VIDEO =
  'https://interactive-examples.mdn.mozilla.net/media/cc0-videos/flower.mp4';

interface HttpsHandleState {
  presented: boolean;
  currentIndex: number;
  currentPath: string;
  completed: { reason: string; index: number } | null;
  completedError: { name: string; message: string } | null;
}

/**
 * `previewMedia` accepts a remote http(s) source without `downloadFile`.
 * The host overlay is native, so this asserts the JS handle: presented while
 * the session is still open, `current.source.path` verbatim, teardown via
 * AbortSignal. Dismissal gestures and desktop window identity are out of
 * scope (PEND-PREVIEW-DISMISS-001).
 */
async function presentHttpsPreview(
  t: Fixture,
  id: string,
  url: string,
  type: 'image' | 'video',
): Promise<void> {
  const { app, namespace, defer } = bindFixture(t, id);
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

  defer(async () => {
    await app.eval({
      script: `
        const s = globalThis[${JSON.stringify(stateKey)}];
        if (s?.controller && !s.controller.signal.aborted) s.controller.abort();
        delete globalThis[${JSON.stringify(stateKey)}];
      `,
    }).catch(() => undefined);
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
