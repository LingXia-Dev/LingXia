import { expect, spec } from '@lingxia/test';
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

interface Outcome {
  threw: boolean;
  code?: string;
  message?: string;
}

desktopSpec('reject haptics and the dialer with E_NOT_SUPPORTED on a desktop', {
  id: 'DEVICE-ABSENT-001',
  covers: ['lx.vibrateShort', 'lx.vibrateLong', 'lx.makePhoneCall'],
  app: SHOWCASE_APP_ID,
  reason: 'absent-proven contract is desktop-only; PEND-DEVICE-FB-001 owns the mobile behaviour',
}, async (t) => {
  const { app } = bindFixture(t, 'DEVICE-ABSENT-001');
  const stackBefore = (await app.nav.stack()).map((page) => page.name);

  const outcomes = await app.eval({
    script: `
      const attempt = (fn) => {
        try { fn(); return { threw: false }; }
        catch (error) { return { threw: true, code: error && error.code, message: String(error && error.message) }; }
      };
      return {
        vibrateShort: attempt(() => lx.vibrateShort()),
        vibrateLong: attempt(() => lx.vibrateLong()),
        makePhoneCall: attempt(() => lx.makePhoneCall({ phoneNumber: '10086' })),
      };
    `,
  }) as Record<'vibrateShort' | 'vibrateLong' | 'makePhoneCall', Outcome>;

  for (const [name, outcome] of Object.entries(outcomes)) {
    // Synchronous, not a rejected promise: a caller with no try/catch sees it immediately.
    expect(outcome.threw).toBe(true);
    expect(`${name}:${outcome.code}`).toBe(`${name}:E_NOT_SUPPORTED`);
  }
  // An unsupported call must not have moved the app anywhere.
  expect((await app.nav.stack()).map((page) => page.name)).toEqual(stackBefore);
});

spec('accept both device orientations and reject an unknown one', {
  id: 'DEVICE-ORIENTATION-001',
  covers: ['lx.setDeviceOrientation'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app, defer } = bindFixture(t, 'DEVICE-ORIENTATION-001');
  defer(async () => {
    await app.eval({ script: `lx.setDeviceOrientation('portrait'); return true;` }).catch(() => undefined);
  });

  const result = await app.eval({
    script: `
      const landscape = lx.setDeviceOrientation('landscape');
      const portrait = lx.setDeviceOrientation('portrait');
      let invalid = null;
      try { lx.setDeviceOrientation('sideways'); }
      catch (error) { invalid = error && error.code; }
      return { landscape, portrait, invalid };
    `,
  }) as { landscape: boolean; portrait: boolean; invalid: string | null };

  expect(result.landscape).toBe(true);
  expect(result.portrait).toBe(true);
  expect(result.invalid).toBe('E_INVALID_ARG');
});
