import { expect, spec } from '@lingxia/test';
import { reveal } from './finger.js';

// An authored size sets a single-line control's height over its default padding and minimum.
// A textbox is the editor inside the field's 1 pt border; a combobox is the bordered box itself.

const sized: Array<[string, 'textbox' | 'combobox']> = [
  ['text-input', 'textbox'], ['date-input', 'textbox'], ['time-input', 'textbox'], ['select', 'combobox'],
];
for (const [page, role] of sized) {
  spec(`${page}: Small, Medium and Large are 32, 40 and 48 pt border to border`, {
    id: `FIELD-HEIGHT-${page.toUpperCase()}-001`, start: { page }, timeout: 60_000,
  }, async (t) => {
    const view = t.app.view;
    const heights: number[] = [];
    for (const name of ['Small', 'Medium', 'Large']) {
      const control = view.getByRole(role, { name });
      const border = role === 'textbox' ? 2 : 0;
      heights.push(Math.round((await reveal(view, control)).height) + border);
    }
    expect(heights).toEqual([32, 40, 48]);
    await t.attach(`${page}-sizes.png`, await view.screenshot());
  });
}
