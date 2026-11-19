import { expect, spec } from '@lingxia/test';
import { openDemo } from './open-demo';

spec('a long list filters as its field is typed in, takes a tap on a row and a fling to its end', { covers: ['view:input', 'view:list'], start: openDemo('filter') }, async (t) => {
  const view = t.app.view;
  const values = view.testId('lab-filter-values');
  await expect(values).toHaveAttribute('data-state', 'query= picked=');
  const search = view.testId('lab-controls-search');
  await search.type('Row 3');
  await expect(values).toHaveAttribute('data-state', 'query=Row 3 picked=');
  // The keyboard can leave the list below the fold: scroll the page by the search field, as a user would.
  const area = await view.viewport();
  await view.swipe({ from: search, to: [area.left + area.width / 2, area.top + 60] });
  await expect(view.testId('lab-controls-row-2')).not.toBeAttached();
  await view.testId('lab-controls-row-31').click();
  await expect(values).toHaveAttribute('data-state', 'query=Row 3 picked=Row 31');

  await search.fill('');
  // The edit can present before the View run that unfilters the list; a fling on the filtered
  // list, already at its end, would hand its distance to the page.
  await expect(values).toHaveAttribute('data-state', 'query= picked=Row 31');
  const list = view.testId('lab-controls-list');
  await expect(list).toBeInViewport();
  await view.fling('up', 3000, { at: list });
  await expect(view.testId('lab-controls-row-40')).toBeInViewport({ timeout: 3_000 });
});

spec('a scroll reports the edges it is at: the start on mount, neither mid-way, the end, both when its rows fit', { covers: ['view:scroll'], start: openDemo('edges') }, async (t) => {
  const view = t.app.view;
  const state = view.testId('lab-edges-state');
  // Already twelve rows: the press only brings the demo on screen.
  await view.testId('lab-edges-many').click();
  await expect(state).toHaveAttribute('data-state', 'start=true end=false');
  const box = await view.testId('lab-edges-scroll').boundingBox();
  const x = box.x + box.width / 2;
  await view.swipe({ from: [x, box.y + box.height - 12], to: [x, box.y + box.height - 52], durationMs: 400 });
  await expect(state).toHaveAttribute('data-state', 'start=false end=false');
  for (let i = 0; i < 8 && (await state.getAttribute('data-state')) !== 'start=false end=true'; i++)
    await view.swipe({ from: [x, box.y + box.height - 12], to: [x, box.y + 12], durationMs: 300 });
  await expect(state).toHaveAttribute('data-state', 'start=false end=true');
  await view.testId('lab-edges-few').click();
  await expect(state).toHaveAttribute('data-state', 'start=true end=true');
  await view.testId('lab-edges-many').click();
  await expect(state).toHaveAttribute('data-state', 'start=true end=false');
});
