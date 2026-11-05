import type { Fixture } from '@lingxia/test';
import { androidDevice } from './android-device.js';

/**
 * Turn the device as a user does: a phone through the device fixture (auto
 * rotation off, the user rotation set), a desktop window by resizing it.
 * `auto` hands a phone back to its sensor and leaves a window as it is.
 */
export function turn(t: Fixture, orientation: 'portrait' | 'landscape' | 'auto'): Promise<void> {
  return t.step(`device.turn ${orientation}`, () => turnNow(t, orientation));
}

const NO_DEVICE = 'turning a phone needs the device fixture: --secret-arg androidDevice=<url>';
const NO_FIXTURE = 'no device fixture turns this phone';
/** Hosts whose window a spec turns by resizing it. */
const DESKTOPS = ['macos', 'windows'];

async function hostOs(t: Fixture): Promise<string> {
  return t.app.logic.eval(({ lx }) => String(lx.host.getBaseInfo().os || '').toLowerCase());
}

async function android(t: Fixture): Promise<boolean> {
  return (await hostOs(t)) === 'android';
}

/**
 * Skip the spec unless the device can be turned. Call it in the spec body before deferring a
 * restore: a skip from cleanup would fail the spec instead.
 */
export async function requireTurn(t: Fixture): Promise<void> {
  const os = await hostOs(t);
  if (os === 'android') {
    if (!t.arg('androidDevice', { required: false })) t.skip(NO_DEVICE);
  } else if (!DESKTOPS.includes(os)) {
    t.skip(NO_FIXTURE);
  }
}

async function turnNow(t: Fixture, orientation: 'portrait' | 'landscape' | 'auto'): Promise<void> {
  if (await android(t)) {
    if (!t.arg('androidDevice', { required: false })) t.skip(NO_DEVICE);
    await androidDevice(t).rotate(orientation);
    return;
  }
  if (orientation !== 'auto') await t.app.window.orient(orientation);
}
