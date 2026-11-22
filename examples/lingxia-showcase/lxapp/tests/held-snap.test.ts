import { expect, test, type Locator, type Fixture } from '@lingxia/test';
import { openDemo } from './open-demo';

// The pager's and panel's index lives in Logic, which answers 250 ms late ("Slow answer"), refuses
// the last page and the panel's top. A swipe must hold where it settled while the answer is pending.

/** Which page sits in the pager's box, sampled until `until` holds or `ms` pass. */
async function pagesSeen(t: Fixture, pager: Locator, ms: number, until: () => Promise<boolean>) {
  const view = t.app.view;
  const box = await pager.boundingBox();
  const seen: number[] = [];
  const deadline = Date.now() + ms;
  while (Date.now() < deadline) {
    for (const index of [0, 1, 2]) {
      const page = await view.testId(`lab-held-page-${index}`).boundingBox().catch(() => null);
      if (page && Math.abs(page.x - box.x) < box.width / 4) { if (seen[seen.length - 1] !== index) seen.push(index); break; }
    }
    if (await until()) break;
  }
  return seen;
}

test('a pager held in Logic waits for its answer, and springs back once from a refused page', {
  id: 'SNAP-HELD-001', covers: ['view:scroll', 'view:snap'], start: openDemo('held'),
}, async (t) => {
  const view = t.app.view;
  const pager = view.testId('lab-held-pager');
  const state = view.testId('lab-held-state');
  await pager.scrollIntoView();
  await expect(state).toHaveAttribute('data-state', 'page=0 panel=1 refused=none');
  await expect(view.testId('lab-held-slow')).toBeChecked();

  const box = await pager.boundingBox();
  const y = box.y + box.height / 2;
  const swipe = () => view.swipe({ from: [box.x + box.width * 0.85, y], to: [box.x + box.width * 0.15, y], durationMs: 300 });

  await swipe();
  const taken = await pagesSeen(t, pager, 2000, async () => (await state.getAttribute('data-state')) === 'page=1 panel=1 refused=none');
  expect(taken, 'never back on page 0 while Logic answers').not.toContain(0);
  await expect(state).toHaveAttribute('data-state', 'page=1 panel=1 refused=none');

  await swipe();
  const refused = await pagesSeen(t, pager, 1500, async () => false);
  // Held on the last page while Logic answers, then back to page 1 once, and it stays.
  const fromLast = refused.slice(refused.indexOf(2));
  expect(fromLast.join('→'), `pages seen ${refused.join('→')}`).toBe('2→1');
  await expect(state).toHaveAttribute('data-state', 'page=1 panel=1 refused=page');
});

test('a dragged panel held in Logic takes an accepted point and springs back from a refused one', {
  id: 'SNAP-HELD-002', covers: ['view:drag', 'view:snap'], start: openDemo('held'),
}, async (t) => {
  const view = t.app.view;
  const panel = view.testId('lab-held-panel');
  const state = view.testId('lab-held-state');
  await panel.scrollIntoView();
  await expect(state).toHaveAttribute('data-state', 'page=0 panel=1 refused=none');
  const rest = (await panel.boundingBox()).y;

  await view.drag(panel, { dx: 0, dy: 48, durationMs: 400 });
  await expect(state).toHaveAttribute('data-state', 'page=0 panel=2 refused=none');
  await expect.poll(async () => Math.round((await panel.boundingBox()).y - rest)).toBe(48);

  await view.drag(panel, { dx: 0, dy: -96, durationMs: 400 });
  // Refused: back to the lowered point, and the page's index never moved.
  await expect.poll(async () => Math.round((await panel.boundingBox()).y - rest)).toBe(48);
  await expect(state).toHaveAttribute('data-state', 'page=0 panel=2 refused=panel');
});
