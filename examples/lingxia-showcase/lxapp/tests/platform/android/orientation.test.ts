import { expect, test } from '@lingxia/test';
import { SHOWCASE_APP_ID } from '@lingxia/test/showcase/helpers/app.js';
import { bindFixture } from '@lingxia/test/showcase/helpers/poll.js';

const platform = globalThis.__LINGXIA_AUTOMATION_HOST__?.control?.platform?.toLowerCase();
test.configure({ skip: platform === 'android' ? undefined : 'Android only' });

type Orientation = 'portrait' | 'landscape';
interface OrientationProbe {
  removed: Orientation[];
  live: Orientation[];
  offRemoved: () => void;
  offLive: () => void;
}

test('rotate the Android viewport for the locking page only, and deliver events only to live listeners', {
  id: 'ANDROID-ORIENTATION-001',
  app: SHOWCASE_APP_ID,
  covers: ['page.orientation.lock', 'page.orientation.unlock', 'lx.device.orientation.watch'],
  timeout: 60_000,
  reason: 'requires the Android host orientation implementation',
}, async (t) => {
  const { app, namespace, defer } = bindFixture(t, 'ANDROID-ORIENTATION-001');
  await app.nav.reset({ page: 'home' });
  const page = await app.page({ name: 'home' });
  const dimensions = async () => {
    const { width, height } = await page.view.viewport();
    return { width, height };
  };
  const initial = await dimensions();
  const original: Orientation = initial.width > initial.height ? 'landscape' : 'portrait';
  defer(async () => {
    await app.logic.eval(({ lx }, key) => {
      const states = globalThis as unknown as Record<string, OrientationProbe>;
      states[key]?.offRemoved();
      states[key]?.offLive();
      delete states[key];
    }, namespace);
    await app.nav.reset({ page: 'home' });
    await app.logic.eval(({ currentPage }) => { currentPage()?.orientation.unlock(); });
    await t.waitFor(dimensions, { until: size => (size.width > size.height) === (original === 'landscape'), timeout: 15000 });
  });
  const rotate = async (orientation: Orientation) => {
    await app.logic.eval(({ currentPage }, value) => { currentPage()?.orientation.lock(value); }, orientation);
    const size = await t.waitFor(dimensions, {
      until: size => size.width > 0 && size.height > 0 && (size.width > size.height) === (orientation === 'landscape'),
      timeout: 15000,
    });
    expect(size.width === size.height).toBe(false);
  };
  await rotate('portrait');
  await app.logic.eval(({ lx }, key) => {
    const state: OrientationProbe = { removed: [], live: [], offRemoved: () => {}, offLive: () => {} };
    state.offRemoved = lx.device.orientation.watch(value => state.removed.push(value));
    state.offLive = lx.device.orientation.watch(value => state.live.push(value));
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

  // The lock stays with home: a pushed page gets its own orientation, and back restores home's.
  await rotate('landscape');
  await app.nav.go({ page: 'device' });
  // Assumes the phone is held upright: the device page declares no pageOrientation.
  const pushed = await app.page({ name: 'device' });
  await t.waitFor(async () => pushed.view.viewport(), { until: size => size.width > 0 && size.height > size.width, timeout: 15000 });
  await app.nav.back();
  await t.waitFor(dimensions, { until: size => size.width > size.height, timeout: 15000 });
});
