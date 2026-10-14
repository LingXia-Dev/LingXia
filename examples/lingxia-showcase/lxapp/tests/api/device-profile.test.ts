import { CaptureUnavailableError, expect, spec } from '@lingxia/test';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { bindFixture } from '../helpers/poll.js';

/**
 * Capability profile for device APIs whose contract differs by platform. On a
 * desktop the haptics and the dialer are absent, and the published contract
 * is a stable synchronous E_NOT_SUPPORTED — that rejection is the behaviour
 * this suite proves. On a phone they are real and belong to the device lab.
 */
const platform = (globalThis.__LINGXIA_AUTOMATION_HOST__?.args ?? {} as Record<string, string>).platform?.toLowerCase();
const DESKTOP = ['macos', 'windows'];
const desktopSpec = platform && DESKTOP.includes(platform) ? spec : spec.skip;

desktopSpec('reject haptics and the dialer with E_NOT_SUPPORTED on a desktop', {
  id: 'DEVICE-ABSENT-001',
  covers: ['lx.vibrateShort', 'lx.vibrateLong', 'lx.makePhoneCall'],
  app: SHOWCASE_APP_ID,
  reason: 'absent-proven contract is desktop-only; PEND-DEVICE-FB-001 owns the mobile behaviour',
}, async (t) => {
  const { app } = bindFixture(t, 'DEVICE-ABSENT-001');
  const stackBefore = (await app.nav.stack()).map((page) => page.name);

  const outcomes = await app.logic.eval(({ lx }) => {
    const attempt = (fn: () => unknown): { threw: boolean; code?: string; message?: string } => {
      try { fn(); return { threw: false }; }
      catch (error) {
        const failure = error as { code?: string; message?: string } | null;
        return { threw: true, code: failure?.code ?? '', message: String(failure?.message) };
      }
    };
    return {
      vibrateShort: attempt(() => lx.vibrateShort()),
      vibrateLong: attempt(() => lx.vibrateLong()),
      makePhoneCall: attempt(() => lx.makePhoneCall({ phoneNumber: '10086' })),
    };
  });

  for (const [name, outcome] of Object.entries(outcomes)) {
    // Synchronous, not a rejected promise: a caller with no try/catch sees it immediately.
    expect(outcome.threw).toBe(true);
    expect(`${name}:${outcome.code}`).toBe(`${name}:E_NOT_SUPPORTED`);
  }
  // An unsupported call must not have moved the app anywhere.
  expect((await app.nav.stack()).map((page) => page.name)).toEqual(stackBefore);
});

/** Orientation is a real setting on a phone and on macOS, and absent on Windows. */
const ORIENTATION_SUPPORT: Record<string, boolean> = {
  macos: true,
  android: true,
  ios: true,
  harmony: true,
  windows: false,
};

spec('accept both device orientations and reject an unknown one', {
  id: 'DEVICE-ORIENTATION-001',
  covers: ['lx.setDeviceOrientation'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app, defer } = bindFixture(t, 'DEVICE-ORIENTATION-001');
  const supported = ORIENTATION_SUPPORT[platform ?? ''] ?? true;
  defer(async () => {
    await app.logic.eval(({ lx }) => { lx.setDeviceOrientation('portrait'); return true; }).catch(() => undefined);
  });

  const result = await app.logic.eval(({ lx }) => {
    // 'sideways' is not an orientation: the probe passes it on purpose.
    const setOrientation = lx.setDeviceOrientation as (value: string) => unknown;
    const attempt = (value: string): { ok: boolean; value?: unknown; code?: string } => {
      try { return { ok: true, value: setOrientation(value) }; }
      catch (error) { return { ok: false, code: (error as { code?: string } | null)?.code }; }
    };
    return {
      landscape: attempt('landscape'),
      portrait: attempt('portrait'),
      invalid: attempt('sideways'),
    };
  });

  if (!supported) {
    // Absent-proven: a host without orientation rejects with a stable code
    // rather than silently pretending it rotated.
    expect(result.landscape.ok).toBe(false);
    expect(result.landscape.code).toBe('E_NOT_SUPPORTED');
    expect(result.portrait.code).toBe('E_NOT_SUPPORTED');
    return;
  }
  expect(result.landscape.value).toBe(true);
  expect(result.portrait.value).toBe(true);
  expect(result.invalid.ok).toBe(false);
  expect(result.invalid.code).toBe('E_INVALID_ARG');
});

/**
 * Whether the frame the user sees turns with setDeviceOrientation: a phone
 * rotates its display; a desktop host accepts the call and keeps its window.
 */
const ROTATES_FRAME: Record<string, boolean> = {
  android: true,
  ios: true,
  harmony: true,
  macos: false,
  windows: false,
};

spec('rotate the captured app window after setDeviceOrientation', {
  id: 'DEVICE-ORIENTATION-FRAME-001',
  covers: ['lx.setDeviceOrientation'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  if (!(ROTATES_FRAME[platform ?? ''] ?? true)) {
    t.skip(`the ${platform} host keeps its window shape on setDeviceOrientation; DEVICE-ORIENTATION-001 owns its contract`);
  }
  const { app, defer } = bindFixture(t, 'DEVICE-ORIENTATION-FRAME-001');
  // The whole app window as the host draws it, so its size is the frame the
  // user sees, not the WebView's.
  const capture = () => t.app.screenshot().catch((error: unknown) => {
    if (error instanceof CaptureUnavailableError) t.skip(error.hint);
    throw error;
  });
  const orient = (value: 'portrait' | 'landscape') =>
    app.logic.eval(({ lx }, next) => lx.setDeviceOrientation(next), value);
  const shape = (shot: { width: number; height: number }) =>
    shot.width > shot.height ? 'landscape' : shot.height > shot.width ? 'portrait' : 'square';

  const before = await capture();
  defer(async () => { await orient('portrait').catch(() => undefined); });

  expect(await orient('portrait')).toBe(true);
  const portrait = await t.waitFor(capture, { until: (shot) => shape(shot) === 'portrait', timeout: 8_000, interval: 500 });
  expect(await orient('landscape')).toBe(true);
  const landscape = await t.waitFor(capture, { until: (shot) => shape(shot) === 'landscape', timeout: 8_000, interval: 500 });

  // The same window turned a quarter: its two sides swap exactly.
  expect([landscape.width, landscape.height]).toEqual([portrait.height, portrait.width]);
  expect(before.width * before.height).toBe(portrait.width * portrait.height);
});
