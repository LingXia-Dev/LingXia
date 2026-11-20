import { expect, test } from '@lingxia/test';
import { reveal, tap, touchMinimum } from './finger.js';

// A press target shorter than the platform minimum takes a finger in a centred band around it,
// with no change to its reported box.

test('text-input: a finger just outside a 32 pt field focuses it', {
  id: 'TOUCH-TARGET-FIELD-001', start: { page: 'text-input' }, timeout: 60_000,
  reason: 'touch targets apply to a real finger on a phone',
}, async (t) => {
  const view = t.app.view;
  const minimum = await touchMinimum(t);
  const small = view.getByRole('textbox', { name: 'Small' });
  const box = await reveal(view, small);
  // The editor fills the 32 pt field inside its 1 pt border.
  expect(Math.round(box.height), 'the small editor is 30 pt').toBe(30);
  const band = (minimum - box.height) / 2;
  // 2 pt inside the band, above the field: on the label's line, not on the field.
  await tap(view, [box.x + box.width / 2, box.y - (band - 2)]);
  await expect(small).toBeFocused();
  expect(Math.round((await small.boundingBox()).height)).toBe(30);
});

test('checkbox-input: a small checkbox row already meets the touch target', {
  id: 'TOUCH-TARGET-CHECKBOX-001', start: { page: 'checkbox-input' }, timeout: 60_000,
  reason: 'a Liuli checkbox presses as its whole row; the 20 pt box case is a host test',
}, async (t) => {
  const view = t.app.view;
  const minimum = await touchMinimum(t);
  const small = view.getByRole('checkbox', { name: 'Small' });
  const box = await reveal(view, small);
  expect(box.height, 'the row is at least the touch target').toBeGreaterThanOrEqual(minimum - 0.5);
  await expect(small).not.toBeChecked();
  await tap(view, [box.x + 4, box.y + box.height / 2]);
  await expect(small).toBeChecked();
});

// Controls drawn smaller than a finger: the box a finger reaches is still the platform's minimum
// in both directions, whatever the drawn box is.
const drawnSmall: Array<[page: string, testId: string, what: string]> = [
  ['breadcrumb', 'deep-more', 'the folded levels button'],
  ['token', 'person-kai-remove', 'a token\'s remove button'],
  ['page-control', 'welcome-1', 'a page dot'],
  ['info-tip', 'side', 'a small info button'],
];

for (const [page, testId, what] of drawnSmall) {
  test(`${page}: ${what} reaches a finger-sized target`, {
    id: `TOUCH-TARGET-REACH-${page.toUpperCase()}`, start: { page }, timeout: 60_000,
    reason: 'touch targets apply to a real finger on a phone',
  }, async (t) => {
    const view = t.app.view;
    const minimum = await touchMinimum(t);
    const control = view.testId(testId);
    const drawn = await reveal(view, control);
    const reach = await control.touchTarget();
    expect(Math.min(drawn.width, drawn.height), 'drawn smaller than the minimum').toBeLessThan(minimum);
    expect(reach.width, 'reach, across').toBeGreaterThanOrEqual(minimum - 0.5);
    expect(reach.height, 'reach, down').toBeGreaterThanOrEqual(minimum - 0.5);
  });
}

// Compact density tightens what is drawn, not what a finger reaches.
test('shape-and-spacing: a compact field and a compact row still reach a finger-sized target', {
  id: 'TOUCH-TARGET-DENSITY-001', start: { page: 'shape-and-spacing' }, timeout: 60_000,
  reason: 'touch targets apply to a real finger on a phone',
}, async (t) => {
  const view = t.app.view;
  const minimum = await touchMinimum(t);
  for (const id of ['compact-name', 'compact-row']) {
    const control = view.testId(id);
    await reveal(view, control);
    const reach = await control.touchTarget();
    expect(reach.height, `${id} reach`).toBeGreaterThanOrEqual(minimum - 0.5);
  }
});
