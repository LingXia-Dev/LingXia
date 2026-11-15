import { expect, spec } from '@lingxia/test';
import { demos, groups } from '../shared/view-lab/text';

// The catalogue of View: an index of groups, each a pushed page of demos in one shape.
spec('the index lists each group with its count and opens it', { covers: ['view:catalogue'], start: { page: 'view' } }, async (t) => {
  const view = t.app.view;
  for (const group of groups) {
    await expect(view.testId(`view-index-${group.id}-count`)).toHaveText(String(Object.keys(demos[group.id]).length));
  }
  await view.testId('view-index-text').click();
  await expect(view.testId('view-group-text')).toBeVisible();
  expect((await t.app.nav.current()).path).toContain('view-text');
});

for (const group of groups) {
  spec(`${group.page} shows every demo of its group and says what is missing`, { covers: ['view:catalogue'], start: { page: group.page } }, async (t) => {
    const view = t.app.view;
    for (const demo of Object.values(demos[group.id])) expect(await view.testId(demo.testId).count()).toBe(1);
    expect(await view.testId('view-group-missing').count()).toBe(1);
  });
}
