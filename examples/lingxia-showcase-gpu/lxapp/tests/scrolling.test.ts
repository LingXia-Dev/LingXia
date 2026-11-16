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
