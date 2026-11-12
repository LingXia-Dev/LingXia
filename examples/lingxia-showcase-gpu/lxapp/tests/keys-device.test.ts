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

spec('a focused editor scrolled off screen keeps focus and the keyboard, and typing brings it back', {
  id: 'KEY-HARMONY-002', covers: ['view:textarea'],
  reason: 'taps the Huawei system keyboard through the Harmony device fixture',
  start: { page: 'view-lab' },
}, async (t) => {
  const view = t.app.view;
  const device = harmonyDevice(t);
  await view.testId('lab-tab-input').click();
  const notes = view.testId('lab-controls-notes');
  await notes.fill('');
  const surface = (await device.nodes()).filter(node => node.type === 'XComponent')
    .reduce((a, b) => ((b.bounds[2] - b.bounds[0]) * (b.bounds[3] - b.bounds[1]) > (a.bounds[2] - a.bounds[0]) * (a.bounds[3] - a.bounds[1]) ? b : a));
  const scale = (surface.bounds[2] - surface.bounds[0]) / (await view.viewport()).width;
  const screen = (x: number, y: number) => [Math.round(surface.bounds[0] + x * scale), Math.round(surface.bounds[1] + y * scale)] as const;
  // A finger brings the editor on screen, then a tap focuses it and raises the keyboard.
  const area = await view.viewport();
  for (let swipe = 0; swipe < 6; swipe++) {
    const at = await notes.boundingBox();
    if (at.y + at.height <= area.top + area.height - 40) break;
    const [x, y1] = screen(area.left + area.width / 2, area.top + area.height - 40);
    const [, y2] = screen(0, area.top + area.height / 2);
    await device.swipe({ x1: x, y1, x2: x, y2, velocity: 600 });
  }
  const box = await notes.boundingBox();
  await device.click(...screen(box.x + box.width / 2, box.y + box.height / 2));
  await expect(notes).toBeFocused();
  const page = view.testId('view-lab');
  await expect.poll(async () => Number(await page.getAttribute('data-keyboard'))).toBeGreaterThan(0);
  // Finger swipes up until the editor leaves the top of the screen.
  for (let swipe = 0; swipe < 6; swipe++) {
    const at = await notes.boundingBox().catch(() => null);
    if (!at || at.y + at.height <= area.top) break;
    const [x, y1] = screen(area.left + area.width / 2, area.top + Math.min(area.height - 40, 400));
    const [, y2] = screen(0, area.top + 40);
    await device.swipe({ x1: x, y1, x2: x, y2, velocity: 1500 });
  }
  await expect(notes).not.toBeInViewport();
  await expect(notes).toBeFocused();
  expect(Number(await page.getAttribute('data-keyboard'))).toBeGreaterThan(0);
  // Injected key events bypass the system keyboard's editor binding, so a finger taps its Q key
  // (left end of the top letter row on the Huawei keyboard).
  const keyboard = Number(await page.getAttribute('data-keyboard'));
  const bottom = Math.max(...(await device.nodes()).map(node => node.bounds[3]));
  await device.click(Math.round(surface.bounds[0] + (surface.bounds[2] - surface.bounds[0]) * 0.06), Math.round(bottom - keyboard * scale * 0.75));
  await expect(view.testId('lab-controls-values')).toContainText('notes=1');
  await expect(notes).toBeInViewport({ timeout: 3_000 });
});
