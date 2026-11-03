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

async function turnNow(t: Fixture, orientation: 'portrait' | 'landscape' | 'auto'): Promise<void> {
  const device = t.arg('androidDevice', { required: false });
  const os = await t.app.logic.eval(({ lx }) => String(lx.host.getBaseInfo().os || '').toLowerCase());
  if (os === 'android') {
    if (!device) t.skip('turning a phone needs the device fixture: --secret-arg androidDevice=<url>');
    await androidDevice(t).rotate(orientation);
    return;
  }
  if (orientation !== 'auto') await t.app.window.orient(orientation);
}
