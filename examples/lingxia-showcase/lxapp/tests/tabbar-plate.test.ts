import { expect, test } from '@lingxia/test';

// A bottom tab bar's plate, translucent ones included, runs on under the system navigation bar to
// the window's edge: no strip of page shows below it.
test('a bottom tab bar plate reaches the window bottom edge', {
  id: 'TABBAR-PLATE-001', covers: ['app:tabBar.style'],
  start: { page: 'home' },
}, async (t) => {
  await expect(t.app.view.testId('home-page')).toBeVisible({ timeout: 30_000 });
  const elements = await t.app.chrome.elements({ scope: 'tabBar' }).catch(() => null);
  const bar = elements?.find((item) => item.role === 'bar');
  if (!bar) return t.skip('the host does not report its tab bar frame yet');
  const window = await t.app.chrome.viewport();
  const [, top, , height] = bar.bounds;
  if (top + height < window.top + window.height * 0.5) t.skip('the tab bar is not on the bottom edge here');
  expect(Math.abs(top + height - (window.top + window.height))).toBeLessThanOrEqual(1);
});
