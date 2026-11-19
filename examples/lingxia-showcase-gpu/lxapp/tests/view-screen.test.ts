import { expect, spec, type Fixture } from '@lingxia/test';
import { openDemo } from './open-demo';

const state = async (t: Fixture, id: string) => (await t.app.view.testId(id).getAttribute('data-state')) ?? '';

/** Every value the screen readout shows, checked for shape; the capsule's raw value is returned. */
async function wellFormed(t: Fixture): Promise<{ capsule: string; safeTop: number }> {
  await expect(t.app.view.testId('lab-env-theme')).toBeVisible();
  for (const key of ['safe', 'insets']) {
    for (const edge of ['top', 'right', 'bottom', 'left']) {
      const value = Number(await state(t, `lab-env-${key}-${edge}`));
      expect(Number.isFinite(value) && value >= 0, `${key} ${edge} = ${value}`).toBe(true);
    }
  }
  expect(['compact', 'regular']).toContain(await state(t, 'lab-env-size-class'));
  expect(['standard', 'high']).toContain(await state(t, 'lab-env-contrast'));
  expect(['light', 'dark']).toContain(await state(t, 'lab-env-theme'));
  expect(['reduce', 'full']).toContain(await state(t, 'lab-env-reduced-motion'));
  const scale = Number(await state(t, 'lab-env-duration-scale'));
  expect(scale >= 0 && scale <= 10, `animator duration scale ${scale}`).toBe(true);
  if (scale === 0) expect(await state(t, 'lab-env-reduced-motion'), 'a zero duration scale reduces motion').toBe('reduce');
  const [vw, vh] = (await state(t, 'lab-env-viewport')).split(',').map(Number);
  const page = await t.app.view.testId('view-screen').boundingBox().catch(() => null);
  expect(vw > 0 && vh > 0, `viewport ${vw}x${vh}`).toBe(true);
  if (page) expect(Math.abs(page.width - vw) <= 1, `viewport width ${vw} is the page's ${page.width}`).toBe(true);
  expect(Number(await state(t, 'lab-env-keyboard')) >= 0).toBe(true);
  expect(['fine', 'coarse']).toContain(await state(t, 'lab-env-pointer'));
  expect(await state(t, 'lab-env-os')).not.toBe('');
  expect(['ltr', 'rtl']).toContain(await state(t, 'lab-env-direction'));
  expect(['h12', 'h23']).toContain(await state(t, 'lab-env-hour-cycle'));
  expect(['gpu', 'driver']).toContain(await state(t, 'lab-env-renderer'));
  const capsule = await state(t, 'lab-env-capsule');
  expect(capsule, 'none, or x,y,width,height').toMatch(/^(none|-?[\d.]+,-?[\d.]+,[\d.]+,[\d.]+)$/);
  return { capsule, safeTop: Number(await state(t, 'lab-env-safe-top')) };
}

spec('a page reads its screen: safe area, insets, size class, contrast, appearance, motion, viewport, keyboard, device and language, and no capsule under a nav bar', {
  covers: ['view:environment'], start: openDemo('screen'),
}, async (t) => {
  const { capsule } = await wellFormed(t);
  expect(capsule, 'a page with a navigation bar has no capsule over it').toBe('none');
});

spec('a page without a nav bar knows where the capsule floats, inside the top band', {
  covers: ['view:environment'], start: { page: 'view-screen', query: { lang: 'en' } },
}, async (t) => {
  const { capsule, safeTop } = await wellFormed(t);
  const { width } = await t.app.view.testId('view-screen').boundingBox();
  if (capsule === 'none') t.skip('this host shows no capsule over a page without a navigation bar');
  const [x, y, w, h] = capsule.split(',').map(Number);
  expect(w > 0 && h > 0, `capsule size ${w}x${h}`).toBe(true);
  expect(x >= 0 && x + w <= width, `capsule ${x}+${w} within the page width ${width}`).toBe(true);
  // The band a navigation bar would fill: from the top edge to a bar's height below the safe area.
  expect(y >= 0 && y + h <= safeTop + 64, `capsule ${y}+${h} inside the top band (safe top ${safeTop})`).toBe(true);
  await expect(t.app.view.testId('view-screen-capsule-outline')).toBeVisible();
  // The header keeps clear of it.
  const header = await t.app.view.testId('view-screen-back').boundingBox();
  expect(header.y >= y + h, `back button at ${header.y}, capsule ends at ${y + h}`).toBe(true);
});

spec('a pinned environment reaches the readout: still motion reads as reduced, the theme and contrast as pinned', {
  covers: ['view:environment'], start: openDemo('screen'),
}, async (t) => {
  await t.app.view.environment({ theme: 'dark', still: true, contrast: 'high' });
  await expect.poll(() => state(t, 'lab-env-reduced-motion')).toBe('reduce');
  expect(await state(t, 'lab-env-theme')).toBe('dark');
  expect(await state(t, 'lab-env-contrast')).toBe('high');
  await t.app.view.environment({ theme: 'light', still: false, contrast: 'standard' });
  await expect.poll(() => state(t, 'lab-env-theme')).toBe('light');
  expect(await state(t, 'lab-env-contrast')).toBe('standard');
});
