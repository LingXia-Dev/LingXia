import { expect, spec } from '@lingxia/test';
import { SHOWCASE_APP_ID } from '../../helpers/app.js';
import { bindFixture } from '../../helpers/poll.js';

const platform = globalThis.__LINGXIA_AUTOMATION_HOST__?.args?.platform?.toLowerCase();
const androidSpec = platform === 'android' ? spec : spec.skip;

type Orientation = 'portrait' | 'landscape';
interface OrientationProbe {
  removed: Orientation[];
  live: Orientation[];
  offRemoved: () => void;
  offLive: () => void;
}

androidSpec('rotate the Android viewport and deliver events only to live listeners', {
  id: 'ANDROID-ORIENTATION-001',
  app: SHOWCASE_APP_ID,
  covers: ['lx.setDeviceOrientation', 'lx.onDeviceOrientationChange'],
  timeout: 60_000,
  reason: 'requires the Android host orientation implementation',
}, async (t) => {
  const { app, namespace, defer } = bindFixture(t, 'ANDROID-ORIENTATION-001');
  await app.nav.relaunch({ page: 'home' });
  const page = await app.page({ name: 'home' });
  const dimensions = () => page.view.eval(({ window }) => ({ width: window.innerWidth, height: window.innerHeight }));
  const initial = await dimensions();
  const original: Orientation = initial.width > initial.height ? 'landscape' : 'portrait';
  defer(async () => {
    await app.logic.eval(({ lx }, key, original) => {
      const states = globalThis as unknown as Record<string, OrientationProbe>;
      states[key]?.offRemoved();
      states[key]?.offLive();
      delete states[key];
      lx.setDeviceOrientation(original);
    }, namespace, original);
    await t.waitFor(dimensions, { until: size => (size.width > size.height) === (original === 'landscape'), timeout: 15000 });
  });
  const rotate = async (orientation: Orientation) => {
    expect(await app.logic.eval(({ lx }, value) => lx.setDeviceOrientation(value), orientation)).toBe(true);
    const size = await t.waitFor(dimensions, {
      until: size => size.width > 0 && size.height > 0 && (size.width > size.height) === (orientation === 'landscape'),
      timeout: 15000,
    });
    expect(size.width === size.height).toBe(false);
  };
  await rotate('portrait');
  await app.logic.eval(({ lx }, key) => {
    const state: OrientationProbe = { removed: [], live: [], offRemoved: () => {}, offLive: () => {} };
    state.offRemoved = lx.onDeviceOrientationChange(event => state.removed.push(event.value));
    state.offLive = lx.onDeviceOrientationChange(event => state.live.push(event.value));
    (globalThis as unknown as Record<string, OrientationProbe>)[key] = state;
  }, namespace);
  const events = () => app.logic.eval((_, key) => {
    const state = (globalThis as unknown as Record<string, OrientationProbe>)[key];
    return { removed: state.removed, live: state.live };
  }, namespace);
  await rotate('landscape');
  const first = await t.waitFor(events, {
    until: state => state.live[state.live.length - 1] === 'landscape'
      && state.removed[state.removed.length - 1] === 'landscape',
  });
  expect(first.removed).toEqual(first.live);
  // The host may deliver its initial portrait snapshot after subscription.
  expect(first.live[0] === 'portrait' ? first.live.slice(1) : first.live).toEqual(['landscape']);
  await app.logic.eval((_, key) => {
    const state = (globalThis as unknown as Record<string, OrientationProbe>)[key];
    state.offRemoved();
    state.offRemoved();
  }, namespace);
  await rotate('portrait');
  // The surviving listener is the delivery barrier for the removed listener.
  await expect.poll(events).toEqual({ removed: first.removed, live: [...first.live, 'portrait'] });
  await expect(page.view.testId('home-page')).toBeVisible();
});
