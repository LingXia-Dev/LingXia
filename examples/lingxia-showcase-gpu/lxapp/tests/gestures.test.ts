import { expect, spec } from '@lingxia/test';

// Drag elements, driven by a finger and by their semantics-tree equivalents.
spec('a drag element follows the finger and lands on the nearest snap point', { covers: ['view:drag'], start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  await view.testId('lab-tab-gesture').click();
  const card = view.testId('lab-drag-card');
  const state = view.testId('lab-drag-state');
  await expect(state).toHaveText('at=0 last=none');
  await view.drag(card, { dx: 120, dy: 0, durationMs: 600 });
  await expect(state).toHaveText('at=1 last=released at 1');
  // Across its axis the finger does not move it.
  await view.drag(card, { dx: 0, dy: 40 });
  await expect(state).toHaveText('at=1 last=released at 1');
  await view.testId('lab-drag-reset').click();
  await expect(state).toHaveText('at=0 last=released at 1');
});

spec('arrow keys and assistive input step a drag element through its points', { covers: ['view:drag', 'view:a11y'], start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  await view.testId('lab-tab-gesture').click();
  const card = view.getByRole('slider', { name: 'Card' });
  await expect(card).toHaveValue('1');
  await card.press('ArrowRight');
  await expect(view.testId('lab-drag-state')).toHaveText('at=1 last=none');
  await expect(card).toHaveValue('2');
  await card.press('End');
  await expect(view.testId('lab-drag-state')).toHaveText('at=2 last=none');
  await card.press('Home');
  await expect(card).toHaveValue('1');
});

spec('a panel opens before its list scrolls and its list returns to the top before it closes', { covers: ['view:drag'], start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  await view.testId('lab-tab-gesture').click();
  const state = view.testId('lab-chain-state');
  await expect(state).toHaveText('at=1');
  // On a phone the panel starts below the fold: scroll the page by a drag on plain text.
  await view.drag(view.testId('lab-drag-state'), { dx: 0, dy: -300, durationMs: 600 });
  // One upward drag from inside the list: the panel opens fully, then the list scrolls.
  await view.drag(view.testId('lab-chain-row-2'), { dx: 0, dy: -300, durationMs: 800 });
  await expect(state).toHaveText('at=0');
  await expect(view.testId('lab-chain-row-1')).not.toBeInViewport();
  // One downward drag: the list reaches its top, then the panel closes.
  await view.drag(view.testId('lab-chain-row-6'), { dx: 0, dy: 360, durationMs: 800 });
  await expect(view.testId('lab-chain-row-1')).toBeInViewport();
  await expect(state).toHaveText('at=1');
});

spec('a sheet moves between its detents and closes when dragged off', { covers: ['view:dialog', 'view:drag'], start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  await view.testId('lab-tab-gesture').click();
  const state = view.testId('lab-detents-state');
  await view.testId('lab-detents-open').click();
  const sheet = view.getByRole('dialog', { name: 'Places' });
  await expect(sheet).toBeVisible();
  await expect(state).toHaveText('open=true at=1 closed=0');
  // Up from the handle area: fully open.
  await view.drag(view.testId('lab-detents-title'), { dx: 0, dy: -260, durationMs: 700 });
  await expect(state).toHaveText('open=true at=0 closed=0');
  // Keys step the detents as assistive tech does.
  await sheet.press('ArrowDown');
  await expect(state).toHaveText('open=true at=1 closed=0');
  // Dragged down past half its height it settles off screen and closes.
  await view.drag(view.testId('lab-detents-title'), { dx: 0, dy: 600, durationMs: 500 });
  await expect(sheet).not.toBeAttached();
  await expect(state).toHaveText('open=false at=2 closed=1');
});

spec('a swipe row opens to its actions, deletes on a full swipe, and offers its actions to assistive tech', { covers: ['view:drag', 'view:a11y'], start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  await view.testId('lab-tab-swipe').click();
  const state = view.testId('lab-swipe-state');
  await expect(state).toHaveText('rows=3 last=none');
  // Closed, the controls behind a row are out of reach for assistive tech.
  await expect(view.getByRole('button', { name: 'Delete' })).toHaveCount(0);
  await view.drag(view.testId('lab-swipe-row-groceries'), { dx: -160, dy: 0, durationMs: 500 });
  await view.testId('lab-swipe-delete-groceries').click();
  await expect(state).toHaveText('rows=2 last=deleted Groceries');
  // The same actions as a screen reader's rotor offers them.
  await view.testId('lab-swipe-row-dentist').performAction('Archive');
  await expect(state).toHaveText('rows=2 last=archived Dentist');
  // A full swipe deletes: past half the row, it heads for its last point.
  const { width } = await view.viewport();
  await view.drag(view.testId('lab-swipe-row-rent'), { dx: -width * 0.85, dy: 0, durationMs: 500 });
  await expect(state).toHaveText('rows=1 last=deleted Rent');
});
