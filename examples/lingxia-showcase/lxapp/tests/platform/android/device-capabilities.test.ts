import { expect, spec } from '@lingxia/test';
import { SHOWCASE_APP_ID } from '../../helpers/app.js';
import { runtimePlatform } from '../../helpers/platform.js';
import { bindFixture, type Caught } from '../../helpers/poll.js';

const testArgs = globalThis.__LINGXIA_AUTOMATION_HOST__?.args ?? {} as Record<string, string>;
const httpBase = testArgs.httpBase;
const platform = testArgs.platform?.toLowerCase();
const androidSpec = platform === 'android' ? spec : spec.skip;
const androidMediaSpec = platform === 'android' && httpBase ? spec : spec.skip;

/**
 * The capabilities a phone really has and a desktop does not. Each one settles
 * on its own — no system dialog, so no external UI driver is involved. What a
 * spec cannot observe (that the motor actually buzzed) is deliberately not
 * asserted; the contract is that the call resolves and the state it reports is
 * real.
 */

interface WifiRoundTrip {
  started: boolean;
  list: { count: number; named: number; sample: string };
  connected: { ssid: string; frequency: number; signal: number };
  listener: string;
  stopped: boolean;
}

androidSpec('scan, read, and observe Wi-Fi on a real radio', {
  id: 'ANDROID-WIFI-001',
  covers: ['lx.startWifi', 'lx.getWifiList', 'lx.getConnectedWifi', 'lx.onWifiConnected', 'lx.stopWifi'],
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
  reason: 'needs a device with Wi-Fi on, location services on, and the location permission granted',
}, async (t) => {
  const { app, defer } = bindFixture(t, 'ANDROID-WIFI-001');
  expect(await runtimePlatform(app)).toBe('android');
  defer(async () => {
    await app.logic.eval(async ({ lx }) => { await lx.stopWifi(); }).catch(() => undefined);
  });

  const result: WifiRoundTrip = await app.logic.eval({ timeout: 30_000 }, async ({ lx }) => {
    await lx.startWifi();
    const list = await lx.getWifiList();
    const connected = await lx.getConnectedWifi();
    // A listener handle must be a function and stay inert after unsubscribe.
    const off = lx.onWifiConnected(() => {});
    const listener = typeof off;
    off();
    off();
    await lx.stopWifi();
    return {
      started: true,
      list: {
        count: list.length,
        named: list.filter((entry) => typeof entry.ssid === 'string').length,
        sample: list[0] ? String(list[0].ssid) : '',
      },
      connected: {
        ssid: String(connected.ssid ?? ''),
        frequency: Number(connected.frequency ?? 0),
        signal: Number(connected.signalStrength ?? -1),
      },
      listener,
      stopped: true,
    };
  });

  // A scan on a real radio sees the networks around it, and every entry is shaped.
  expect(result.list.count).toBeGreaterThan(0);
  expect(result.list.named).toBe(result.list.count);
  // The device under test is on Wi-Fi, so the connected network is real.
  expect(result.connected.ssid.length).toBeGreaterThan(0);
  expect(result.connected.frequency).toBeGreaterThan(0);
  expect(result.connected.signal).toBeGreaterThan(0);
  expect(result.listener).toBe('function');
  expect(result.stopped).toBe(true);
});

androidMediaSpec('save an image and a video into the photo library', {
  id: 'ANDROID-PHOTOS-001',
  covers: ['lx.saveImageToPhotosAlbum', 'lx.saveVideoToPhotosAlbum', 'lx.downloadFile'],
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
  reason: 'needs the HTTP fixture reachable from the device (adb reverse) and an Android host',
}, async (t) => {
  const { app } = bindFixture(t, 'ANDROID-PHOTOS-001');
  expect(await runtimePlatform(app)).toBe('android');

  // Android 10+ writes an app's own media through MediaStore with no permission
  // prompt, so this is a complete contract with no external UI.
  const saved = await app.logic.eval({ timeout: 40_000 }, async ({ lx }, base) => {
    const png = await lx.downloadFile({ url: base + '/media/sample.png' }).result;
    const mp4 = await lx.downloadFile({ url: base + '/media/sample.mp4' }).result;
    await lx.saveImageToPhotosAlbum({ filePath: png.uri });
    await lx.saveVideoToPhotosAlbum({ filePath: mp4.uri });
    return { image: png.sizeBytes, video: mp4.sizeBytes };
  }, httpBase ?? '');
  expect(saved.image).toBeGreaterThan(0);
  expect(saved.video).toBeGreaterThan(0);

  await t.step('a missing source rejects instead of reporting a save', async () => {
    const rejected: Caught = await app.logic.eval(async ({ lx }) => {
      try {
        return { ok: true, value: await lx.saveImageToPhotosAlbum({ filePath: 'lx://temp/does-not-exist.png' }) };
      } catch (error) {
        const { code, message, data } = error as { code?: string; message?: string; data?: unknown };
        return { ok: false, code, message: String(message ?? error), data };
      }
    });
    expect(rejected.ok).toBeFalsy();
    expect(typeof rejected.code).toBe('string');
  });
});

androidSpec('fire both haptics without a dialog', {
  id: 'ANDROID-HAPTICS-001',
  covers: ['lx.vibrateShort', 'lx.vibrateLong'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'ANDROID-HAPTICS-001');
  expect(await runtimePlatform(app)).toBe('android');

  // Whether the motor moved is not observable from here; that the call is
  // supported and resolves — where a desktop rejects it — is.
  const result = await app.logic.eval(async ({ lx }) => {
    const short: unknown = await lx.vibrateShort();
    const long: unknown = await lx.vibrateLong();
    return { short, long };
  });
  expect(result.short).toBe(true);
  expect(result.long).toBe(true);
});
