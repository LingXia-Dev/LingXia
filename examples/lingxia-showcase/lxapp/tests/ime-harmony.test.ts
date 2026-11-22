import { expect, test } from '@lingxia/test';
import { harmonyDevice } from '@lingxia/test/showcase/helpers/harmony-device.js';

test.configure({ requires: { args: ['harmonyDevice'] } });

// Typed text keeps reaching a text field after a numeric one held the input method: each
// focus attaches afresh with that field's keyboard, through real taps on the system keyboard.
// Injected key events bypass the keyboard's editor binding, so a finger taps the letter keys;
// the keyboard must be in Latin mode, where a tap commits its letter instead of composing.
test('a text field after a numeric one still takes typed letters', {
  id: 'IME-HARMONY-001', covers: ['view:input'],
  reason: 'taps the Huawei system keyboard through the Harmony device fixture',
  start: { page: 'view-input' },
}, async (t) => {
  const view = t.app.view;
  const device = harmonyDevice(t);
  const surface = (await device.nodes()).filter(node => node.type === 'XComponent')
    .reduce((a, b) => ((b.bounds[2] - b.bounds[0]) * (b.bounds[3] - b.bounds[1]) > (a.bounds[2] - a.bounds[0]) * (a.bounds[3] - a.bounds[1]) ? b : a));
  const scale = (surface.bounds[2] - surface.bounds[0]) / (await view.viewport()).width;
  const { width, height } = await view.viewport();
  // The layout dump clips the surface under the status bar; the page draws from its bottom edge up.
  const top = surface.bounds[3] - height * scale;
  const page = view.testId('view-group-input');
  const tap = async (testId: string) => {
    const target = view.testId(testId);
    // The viewport keeps its height under a raised keyboard; the keyboard covers its bottom, so
    // the visible band (and every swipe) stays above it: the upper half while none is up.
    const band = async () => {
      const area = await view.viewport();
      const keyboard = Number(await page.getAttribute('data-keyboard')) || 0;
      const visible = area.height - keyboard;
      return { top: area.top, visible, low: area.top + (keyboard > 0 ? visible : area.height * 0.45) };
    };
    let box = await target.boundingBox();
    let shown = await band();
    for (let swipe = 0; swipe < 12 && (box.y < shown.top + shown.visible * 0.1 || box.y + box.height > shown.low); swipe++) {
      const down = box.y + box.height > shown.low;
      const at = (fraction: number) => shown.top + shown.visible * fraction;
      await view.swipe({ from: [width / 2, at(down ? 0.7 : 0.3)], to: [width / 2, at(down ? 0.4 : 0.6)], durationMs: 300 });
      box = await target.boundingBox();
      shown = await band();
    }
    await device.click(Math.round(surface.bounds[0] + (box.x + box.width / 2) * scale),
      Math.round(top + (box.y + box.height / 2) * scale));
    await expect(target).toBeFocused();
  };
  const name = view.testId('lab-input-name');
  // Q, W and E: the first three keys of the top letter row, which spans the screen in ten keys.
  let typed = '';
  for (const [letter, column] of [['q', 0], ['w', 1], ['e', 2]] as const) {
    await tap('lab-key-numeric');
    await tap('lab-input-name');
    await expect.poll(async () => Number(await page.getAttribute('data-keyboard'))).toBeGreaterThan(0);
    const keyboard = Number(await page.getAttribute('data-keyboard'));
    const bottom = Math.max(...(await device.nodes()).map(node => node.bounds[3]));
    const keyWidth = (surface.bounds[2] - surface.bounds[0]) / 10;
    await device.click(Math.round(surface.bounds[0] + keyWidth * (column + 0.5)), Math.round(bottom - keyboard * scale * 0.75));
    typed += letter;
    // The keyboard capitalises the start of a field.
    await expect(name).toHaveValue(new RegExp(`^${typed}$`, 'i'));
  }
});
