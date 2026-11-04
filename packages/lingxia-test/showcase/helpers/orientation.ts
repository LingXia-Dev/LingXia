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

async function android(t: Fixture): Promise<boolean> {
  return (await t.app.logic.eval(({ lx }) => String(lx.host.getBaseInfo().os || '').toLowerCase())) === 'android';
}

/**
 * Skip the spec unless the device can be turned. Call it in the spec body before deferring a
 * restore: a skip from cleanup would fail the spec instead.
 */
export async function requireTurn(t: Fixture): Promise<void> {
  if (!t.arg('androidDevice', { required: false }) && await android(t)) t.skip(NO_DEVICE);
}

async function turnNow(t: Fixture, orientation: 'portrait' | 'landscape' | 'auto'): Promise<void> {
  if (await android(t)) {
    if (!t.arg('androidDevice', { required: false })) t.skip(NO_DEVICE);
    await androidDevice(t).rotate(orientation);
    return;
  }
  if (orientation !== 'auto') await t.app.window.orient(orientation);
}
