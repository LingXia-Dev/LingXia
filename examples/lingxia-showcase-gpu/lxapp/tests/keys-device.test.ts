import { expect, spec } from '@lingxia/test';
import { harmonyDevice } from '@lingxia/test/showcase/helpers/harmony-device.js';

spec.configure({ requires: { args: ['harmonyDevice'] } });

// The same menu walk as the automation-driven menu spec, but with key presses through the
// system's input path, so it checks that hardware keys reach the page at all.
spec('hardware keys move a menu focus and choose an item', {
  id: 'KEY-HARMONY-001', covers: ['view:popover', 'view:a11y'],
  reason: 'presses keys through the Harmony device fixture',
  start: { page: 'view-lab' },
}, async (t) => {
  const view = t.app.view;
  const device = harmonyDevice(t);
  await view.testId('lab-tab-feedback').click();
  // A finger on the page gives it ArkUI focus, which keys need; automation clicks do not move it.
  const surface = (await device.nodes()).filter(node => node.type === 'XComponent')
    .reduce((a, b) => ((b.bounds[2] - b.bounds[0]) * (b.bounds[3] - b.bounds[1]) > (a.bounds[2] - a.bounds[0]) * (a.bounds[3] - a.bounds[1]) ? b : a));
  const scale = (surface.bounds[2] - surface.bounds[0]) / (await view.viewport()).width;
  const last = await view.testId('lab-overlay-last').boundingBox();
  await device.click(Math.round(surface.bounds[0] + (last.x + last.width / 2) * scale),
    Math.round(surface.bounds[1] + (last.y + last.height / 2) * scale));
  // Keyboard only from here: Tab to the menu button; Enter opens the menu on its first item.
  const more = view.testId('lab-overlay-more');
  const tabTo = async (target: typeof more, what: string) => {
    for (let tab = 0; ; tab++) {
      if (tab === 30) throw new Error(`Tab never reached ${what}`);
      await device.key('tab');
      try { await expect(target).toBeFocused({ timeout: 400 }); return; } catch { /* not yet */ }
    }
  };
  await tabTo(more, 'the menu button');
  await device.key('enter');
  const item = (name: string) => view.getByRole('menuitem', { name });
  await expect(item('Rename')).toBeFocused();
  await device.key('end');
  await expect(item('Delete')).toBeFocused();
  await device.key('down');
  await expect(item('Rename')).toBeFocused();
  await device.key('s');
  await expect(item('Share')).toBeFocused();
  await device.key('home');
  await expect(item('Rename')).toBeFocused();
  await device.key('enter');
  await expect(view.testId('lab-overlay-last')).toHaveText('last: Rename');
  await expect(more).toBeFocused();
});
