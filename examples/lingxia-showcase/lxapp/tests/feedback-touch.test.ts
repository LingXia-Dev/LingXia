import { expect, test, type Fixture } from '@lingxia/test';

/**
 * A press on a feedback-layer element, injected at window points: the platform's input dispatch
 * routes it as it routes a finger or the mouse. Skips where the host cannot inject input, or
 * where the layer does not cover the window, so its points are not window points.
 */
async function pressOnLayer(t: Fixture, testId: string) {
  const element = t.app.feedback.view.testId(testId);
  await element.first().waitFor({ state: 'visible', timeout: 30_000 });
  // A sheet slides in from below; its box is only where a finger lands once the entrance ends.
  await t.app.feedback.view.settled();
  const window = await t.app.chrome.viewport().catch(() => null);
  const box = await element.boundingBox();
  if (!window) return t.skip('the host does not report its window, so a layer point cannot be placed in it');
  const layer = await t.app.feedback.view.testId('lx-feedback').boundingBox().catch(() => null);
  if (layer && (Math.abs(layer.width - window.width) > 1 || Math.abs(layer.height - window.height) > 1)) {
    t.skip('the feedback layer covers part of the window (a desktop page card); window points are offset there');
  }
  try {
    await t.app.gesture.tap([box.x + box.width / 2, box.y + box.height / 2]);
  } catch (error) {
    if ((error as { code?: string }).code === 'E_UNSUPPORTED') t.skip('the host cannot inject input');
    throw error;
  }
}

// An automation click reaches a dialog through its semantics; a press goes through the window
// system, which passes over a window that does not take input.
test('a press answers an action sheet and a confirm on the feedback layer', {
  id: 'UI-FEEDBACK-PRESS-001', covers: ['lx.ui.actionSheet', 'lx.ui.confirm'],
  start: { page: 'ui', params: { type: 'actionsheet' } },
}, async (t) => {
  const app = t.app;
  const picked = app.logic.eval(({ lx }) =>
    lx.ui.actionSheet({ items: ['View Details', 'Send Email'].map((label) => ({ id: label, label })) }));
  await pressOnLayer(t, 'lx-action-sheet-item-1');
  const selected = await picked;
  expect(selected.status === 'ok' ? selected.id : selected.status).toBe('Send Email');
  await expect(app.feedback.view.testId('lx-action-sheet')).toHaveCount(0, { timeout: 10_000 });

  const confirmed = app.logic.eval(({ lx }) => lx.ui.confirm({ title: 'Press', message: 'Confirm with a press' }));
  await pressOnLayer(t, 'lx-modal-confirm');
  expect(await confirmed).toBe(true);
});
