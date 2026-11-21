import { expect, test } from '@lingxia/test';
import { harmonyDevice } from '@lingxia/test/showcase/helpers/harmony-device.js';

test.configure({ requires: { args: ['harmonyDevice'] } });

// Typed text keeps reaching a text field after a numeric one held the input method: each
// focus attaches afresh with that field's keyboard, through real taps and key presses.
test('a text field after a numeric one still takes typed letters', {
  id: 'IME-HARMONY-001', covers: ['view:input'],
  reason: 'taps and types through the Harmony device fixture',
  start: { page: 'view-input', params: { lang: 'en' } },
}, async (t) => {
  const view = t.app.view;
  const device = harmonyDevice(t);
  const surface = (await device.nodes()).filter(node => node.type === 'XComponent')
    .reduce((a, b) => ((b.bounds[2] - b.bounds[0]) * (b.bounds[3] - b.bounds[1]) > (a.bounds[2] - a.bounds[0]) * (a.bounds[3] - a.bounds[1]) ? b : a));
  const scale = (surface.bounds[2] - surface.bounds[0]) / (await view.viewport()).width;
  const { width, height } = await view.viewport();
  // The layout dump clips the surface under the status bar; the page draws from its bottom edge up.
  const top = surface.bounds[3] - height * scale;
  const tap = async (testId: string) => {
    const target = view.testId(testId);
    // Swipe the field to the upper half, clear of where the keyboard rises.
    let box = await target.boundingBox();
    for (let swipe = 0; swipe < 12 && (box.y < height * 0.15 || box.y > height * 0.45); swipe++) {
      const down = box.y > height * 0.45;
      await view.swipe({ from: [width / 2, height * (down ? 0.7 : 0.3)], to: [width / 2, height * (down ? 0.4 : 0.6)], durationMs: 300 });
      box = await target.boundingBox();
    }
    await device.click(Math.round(surface.bounds[0] + (box.x + box.width / 2) * scale),
      Math.round(top + (box.y + box.height / 2) * scale));
    await expect(target).toBeFocused();
  };
  const name = view.testId('lab-input-name');
  let typed = '';
  for (const letter of ['a', 'b', 'c'] as const) {
    await tap('lab-key-numeric');
    await tap('lab-input-name');
    await device.key(letter);
    typed += letter;
    await expect(name).toHaveValue(typed);
  }
});
