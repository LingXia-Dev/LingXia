import { expect, test } from '@lingxia/test';

// A closed Details keeps its body in the page, clipped to nothing and hidden: nothing in it may be
// reached. Tab from the closed row's summary goes past the body's control.
test('details: Tab from a closed row skips the control inside its body', {
  id: 'DETAILS-CLOSED-FOCUS-001', start: { page: 'details' }, timeout: 60_000,
}, async (t) => {
  const view = t.app.view;
  const summary = view.testId('form-summary');
  const inside = view.testId('form-name');
  await summary.press('Tab');
  await expect(inside).not.toBeFocused();
  await summary.click();
  await expect(inside).toBeVisible();
});
