import { CaptureUnavailableError, expect, spec } from '@lingxia/test';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { bindFixture } from '../helpers/poll.js';

/**
 * `t.app.screenshot()` is taken by the host in process, so it needs no
 * permission anywhere; `t.device.screenshot()` sees the whole display and,
 * on macOS, needs Screen Recording for the app that launched the host.
 */

spec('capture the host window in process and attach it', {
  id: 'APP-CAPTURE-001',
  app: SHOWCASE_APP_ID,
}, async (t) => {
  // Two captures in a row of the same size: the window is not mid-rotation.
  const [, shot] = await t.waitFor(async () => [await t.app.screenshot(), await t.app.screenshot()] as const, {
    until: ([a, b]) => a.width === b.width && a.height === b.height,
    timeout: 8_000,
    interval: 300,
  });
  expect(shot.path).toMatch(/^attachments\/.+\/app-\d+\.png$/);
  expect(shot.scale >= 1).toBe(true);
  // At least a phone-sized window, in points.
  expect(Math.min(shot.width, shot.height) / shot.scale >= 240).toBe(true);
  expect(Math.max(shot.width, shot.height) / shot.scale >= 320).toBe(true);

  // The active main surface is drawn in the current page's window.
  const { activeMainId } = await t.app.surfaceLayout();
  if (activeMainId) {
    const main = await t.app.screenshot({ surface: activeMainId });
    expect([main.width, main.height]).toEqual([shot.width, shot.height]);
  }
  const unknown = await t.app.screenshot({ surface: 'no-such-surface' }).catch((error: { code?: string }) => error);
  expect((unknown as { code?: string }).code).toBe('E_AUTOMATION');
});

spec('capture the window a float surface is shown in', {
  id: 'APP-CAPTURE-002',
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app, namespace, defer } = bindFixture(t, 'APP-CAPTURE-002');
  defer(async () => {
    await app.logic.eval(async ({ lx }, key) => {
      const surface = lx.surface.getByKey(key);
      if (surface?.kind === 'page') await surface.close();
    }, namespace);
  });
  const host = await t.app.screenshot();
  const float = await app.logic.eval(async ({ lx }, key) =>
    (await lx.surface.openPage('surface', { as: 'float', key, query: { fixture: key } })).id, namespace);
  await expect((await app.page({ name: 'surface' })).view.testId('surface-page')).toBeVisible();
  const shot = await t.app.screenshot({ surface: float });
  // A desktop float is a window of its own, inside the host's bounds; a
  // phone draws it in the one app window.
  expect(shot.width <= host.width && shot.height <= host.height).toBe(true);
});

spec('capture the whole display, or say why it cannot', {
  id: 'DEVICE-CAPTURE-001',
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const display = await t.device.screenshot().catch((error: unknown) => {
    if (error instanceof CaptureUnavailableError) {
      expect(error.hint.length > 0).toBe(true);
      t.skip(error.hint);
    }
    throw error;
  });
  expect(display.path).toMatch(/^attachments\/.+\/device-\d+\.png$/);
  const window = await t.app.screenshot();
  // The app's window lies on the display: it can never hold more pixels.
  expect(display.width * display.height >= window.width * window.height * 0.95).toBe(true);
});
