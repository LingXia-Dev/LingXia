import { expect, test } from '@lingxia/test';
import { openDemo } from './open-demo';

// Drag elements, driven by a finger and by their semantics-tree equivalents.
test('a drag element follows the finger and lands on the nearest snap point', { covers: ['view:drag'], start: openDemo('stops') }, async (t) => {
  const view = t.app.view;
  const card = view.testId('lab-drag-card');
  const state = view.testId('lab-drag-state');
  await expect(state).toHaveAttribute('data-state', 'at=0 last=none');
  await view.drag(card, { dx: 120, dy: 0, durationMs: 600 });
  await expect(state).toHaveAttribute('data-state', 'at=1 last=released at 1');
  // Across its axis the finger does not move it.
  await view.drag(card, { dx: 0, dy: 40 });
  await expect(state).toHaveAttribute('data-state', 'at=1 last=released at 1');
  await view.testId('lab-drag-reset').click();
  await expect(state).toHaveAttribute('data-state', 'at=0 last=released at 1');
});

test('arrow keys and assistive input step a drag element through its points', { covers: ['view:drag', 'view:a11y'], start: openDemo('stops') }, async (t) => {
  const view = t.app.view;
  const card = view.getByRole('slider', { name: 'Card' });
  await expect(card).toHaveValue('1');
  await card.press('ArrowRight');
  await expect(view.testId('lab-drag-state')).toHaveAttribute('data-state', 'at=1 last=none');
  await expect(card).toHaveValue('2');
  await card.press('End');
  await expect(view.testId('lab-drag-state')).toHaveAttribute('data-state', 'at=2 last=none');
  await card.press('Home');
  await expect(card).toHaveValue('1');
});

test('a panel opens before its list scrolls and its list returns to the top before it closes', { covers: ['view:drag'], start: openDemo('chain') }, async (t) => {
  const view = t.app.view;
  const state = view.testId('lab-chain-state');
  await expect(state).toHaveAttribute('data-state', 'at=1');
  // On a phone the panel starts below the fold: scroll the page by a drag on plain text.
  await view.drag(view.testId('lab-drag-state'), { dx: 0, dy: -300, durationMs: 600 });
  // One upward drag from inside the list: the panel opens fully, then the list scrolls.
  await view.drag(view.testId('lab-chain-row-2'), { dx: 0, dy: -300, durationMs: 800 });
  await expect(state).toHaveAttribute('data-state', 'at=0');
  await expect(view.testId('lab-chain-row-1')).not.toBeInViewport();
  // One downward drag: the list reaches its top, then the panel closes.
  await view.drag(view.testId('lab-chain-row-6'), { dx: 0, dy: 360, durationMs: 800 });
  await expect(view.testId('lab-chain-row-1')).toBeInViewport();
  await expect(state).toHaveAttribute('data-state', 'at=1');
});

test('a sheet moves between its detents and closes when dragged off', { covers: ['view:dialog', 'view:drag'], start: openDemo('sheet') }, async (t) => {
  const view = t.app.view;
  const state = view.testId('lab-detents-state');
  await view.testId('lab-detents-open').click();
  const sheet = view.getByRole('dialog', { name: 'Places' });
  await expect(sheet).toBeVisible();
  await expect(state).toHaveAttribute('data-state', 'open=true at=1 closed=0');
  // Up from the handle area: fully open.
  await view.drag(view.testId('lab-detents-title'), { dx: 0, dy: -260, durationMs: 700 });
  await expect(state).toHaveAttribute('data-state', 'open=true at=0 closed=0');
  // Keys step the detents as assistive tech does.
  await sheet.press('ArrowDown');
  await expect(state).toHaveAttribute('data-state', 'open=true at=1 closed=0');
  // Dragged down past half its height it settles off screen and closes.
  await view.drag(view.testId('lab-detents-title'), { dx: 0, dy: 600, durationMs: 500 });
  await expect(sheet).not.toBeAttached();
  await expect(state).toHaveAttribute('data-state', 'open=false at=2 closed=1');
  // Reopened, it rests on its half detent again, not where the drag left it.
  await view.testId('lab-detents-open').click();
  await expect(state).toHaveAttribute('data-state', 'open=true at=1 closed=1');
  await expect(sheet).toBeInViewport();
});

test('a swipe row opens to its actions, deletes on a full swipe, and offers its actions to assistive tech', { covers: ['view:drag', 'view:a11y'], start: openDemo('rows') }, async (t) => {
  const view = t.app.view;
  const state = view.testId('lab-swipe-state');
  await expect(state).toHaveAttribute('data-state', 'rows=3 last=none');
  // Closed, the controls behind a row are out of reach for assistive tech.
  await expect(view.getByRole('button', { name: 'Delete' })).toHaveCount(0);
  await view.drag(view.testId('lab-swipe-row-groceries'), { dx: -160, dy: 0, durationMs: 500 });
  await view.testId('lab-swipe-delete-groceries').click();
  await expect(state).toHaveAttribute('data-state', 'rows=2 last=deleted Groceries');
  // The same actions as a screen reader's rotor offers them.
  await view.testId('lab-swipe-row-dentist').performAction('Archive');
  await expect(state).toHaveAttribute('data-state', 'rows=2 last=archived Dentist');
  // A full swipe deletes: past half the row, it heads for its last point.
  const { width } = await view.viewport();
  await view.drag(view.testId('lab-swipe-row-rent'), { dx: -width * 0.85, dy: 0, durationMs: 500 });
  await expect(state).toHaveAttribute('data-state', 'rows=1 last=deleted Rent');
});

test('a pager stops on the next page however fast the flick, and its bound page moves it', { covers: ['view:scroll', 'view:a11y'], start: openDemo('pages') }, async (t) => {
  const view = t.app.view;
  const state = view.testId('lab-pager-state');
  await expect(state).toHaveAttribute('data-state', 'page=0');
  await view.fling('left', 4000, { at: view.testId('lab-pager-scroll') });
  await expect(state).toHaveAttribute('data-state', 'page=1');
  await view.testId('lab-pager-next').click();
  await expect(state).toHaveAttribute('data-state', 'page=2');
  // A screen reader's scroll steps one page.
  await view.testId('lab-pager-scroll').performAction('decrement');
  await expect(state).toHaveAttribute('data-state', 'page=1');
});

test('the kit pager reads as a list of pages, its dots follow the page and a tap on one jumps there', { covers: ['view:scroll', 'view:a11y'], start: openDemo('dots') }, async (t) => {
  const view = t.app.view;
  const state = view.testId('lab-kit-pager-state');
  await expect(state).toHaveAttribute('data-state', 'page=0');
  // A screen reader meets the pages as one named list of three; the dots are not in its tree.
  await expect(view.getByRole('list', { name: 'Checkout steps' })).toHaveCount(1);
  await expect(view.getByRole('listitem')).toHaveCount(3);
  await expect(view.getByRole('button', { name: /^$/ })).toHaveCount(0);
  const width = async (index: number) => (await view.testId(`lab-kit-pager-view-mark-${index}`).boundingBox()).width;
  await expect.poll(() => width(0)).toBe(18);
  // A screen reader's scroll steps one page and the dots follow it.
  await view.testId('lab-kit-pager-view-scroll').performAction('increment');
  await expect(state).toHaveAttribute('data-state', 'page=1');
  await expect.poll(() => width(1)).toBe(18);
  // The dots are out of the accessibility tree, so a finger taps one once the pager settles.
  const left = async (id: string) => (await view.testId(id).boundingBox()).x;
  await expect.poll(async () => await left('lab-kit-pager-view-page-1') - await left('lab-kit-pager-view-scroll')).toBe(0);
  await (await view.touch(view.testId('lab-kit-pager-view-dot-2'))).up();
  await expect(state).toHaveAttribute('data-state', 'page=2');
  await expect.poll(() => width(2)).toBe(18);
  await expect.poll(() => width(1)).toBe(6);
  await expect.poll(async () => await left('lab-kit-pager-view-page-2') - await left('lab-kit-pager-view-scroll')).toBe(0);
  await view.fling('right', 4000, { at: view.testId('lab-kit-pager-view-scroll') });
  await expect(state).toHaveAttribute('data-state', 'page=1');
  await expect.poll(() => width(1)).toBe(18);
  await expect.poll(() => width(2)).toBe(6);
});

test('a tap on a nested switch, a row button and a rich-text link reaches only that control', { covers: ['view:switch', 'view:link', 'view:button'], start: openDemo('nested') }, async (t) => {
  const view = t.app.view;
  const state = view.testId('lab-nested-state');
  await expect(state).toHaveAttribute('data-state', 'opened=0 wifi=true removed=0');
  await view.testId('lab-nested-switch').click();
  await expect(state).toHaveAttribute('data-state', 'opened=0 wifi=false removed=0');
  await view.testId('lab-nested-remove').click();
  await expect(state).toHaveAttribute('data-state', 'opened=0 wifi=false removed=1');

  await t.app.nav.go(openDemo('runs'));
  await view.testId('lab-rich-link').click();
  await expect(view.testId('lab-rich-presses')).toHaveAttribute('data-state', 'Link pressed 1 times');
});

test('a mouse crossing nested boxes calls enter outermost first and leave innermost first', { start: openDemo('hover') }, async (t) => {
  const os = await t.app.logic.eval(({ lx }) => String(lx.host.info.os || '').toLowerCase());
  if (os !== 'macos' && os !== 'windows') t.skip('needs a mouse: desktop pointer input');
  const view = t.app.view;
  const log = view.testId('lab-crossing-log');
  // Mouse events go to the host window inside the app: the system pointer stays where it is and
  // the window is never focused.
  const clear = (await view.getByRole('button', { name: 'Clear' }).boundingBox())!;
  const at = (dx: number): [number, number] => [Math.round(clear.x + clear.width / 2 + dx), Math.round(clear.y + clear.height / 2)];
  const away: [number, number] = [4, 4];
  const move = (point: [number, number]) => t.app.window.pointer.move({ at: point });
  await move(away);
  await expect(log).toHaveAttribute('data-state', 'none');
  // Onto the inner button: the outer box enters first.
  await move(at(0));
  await expect(log).toHaveAttribute('data-state', 'O+I+');
  // Off the button but inside the box (the gap before the log text): only the button leaves.
  await move(at(clear.width / 2 + 6));
  await expect(log).toHaveAttribute('data-state', 'O+I+I-');
  await move(at(0));
  await move(away);
  await expect(log).toHaveAttribute('data-state', 'O+I+I-I+I-O-');
});
