import { expect, spec } from '@lingxia/test';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { bindFixture } from '../helpers/poll.js';
import { turn } from '../helpers/orientation.js';

const wide = async (view: { viewport(): Promise<{ width: number; height: number }> }) => {
  const { width, height } = await view.viewport();
  return width > height;
};

spec('a page that follows the device turns with it', {
  id: 'ORIENT-AUTO-001',
  covers: ['page:pageOrientation=auto'],
  app: SHOWCASE_APP_ID,
  requires: { input: ['touch', 'mouse'] },
}, async (t) => {
  const { app } = bindFixture(t, 'ORIENT-AUTO-001');
  await app.nav.relaunch({ page: 'home' });
  const view = app.view;
  await view.testId('home-page').waitFor({ timeout: 30_000 });
  const start = await wide(view);
  t.defer(async () => { await turn(t, 'auto'); if (!start) await turn(t, 'portrait'); });

  await turn(t, 'landscape');
  await expect.poll(() => wide(view), { timeout: 10_000 }).toBe(true);
  await expect(view.testId('home-page')).toBeVisible();
  await turn(t, 'portrait');
  await expect.poll(() => wide(view), { timeout: 10_000 }).toBe(false);
  await expect(view.testId('home-page')).toBeVisible();
});

spec('a landscape page lies down while the phone is upright, and the next page stands up again', {
  id: 'ORIENT-LANDSCAPE-001',
  covers: ['page:pageOrientation=landscape'],
  app: SHOWCASE_APP_ID,
  // A desktop window keeps the shape its user gave it.
  requires: { input: ['touch'] },
}, async (t) => {
  const { app } = bindFixture(t, 'ORIENT-LANDSCAPE-001');
  t.defer(async () => { await turn(t, 'auto'); });
  await turn(t, 'portrait');
  await app.nav.relaunch({ page: 'home' });
  await expect.poll(() => wide(app.view), { timeout: 10_000 }).toBe(false);
  await app.nav.to({ page: 'wifi' });
  await expect.poll(() => wide(app.view), { timeout: 10_000 }).toBe(true);
  await app.nav.back();
  await expect.poll(() => wide(app.view), { timeout: 10_000 }).toBe(false);
});
