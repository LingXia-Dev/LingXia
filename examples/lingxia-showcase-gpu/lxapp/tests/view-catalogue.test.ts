import { expect, spec } from '@lingxia/test';
import { demos, groups } from '../shared/view-lab/text';

// The catalogue of View: an index of groups, each a pushed page of demos in one shape.
spec('the index lists each group with its count and opens it', { covers: ['view:catalogue'], start: { page: 'view' } }, async (t) => {
  const view = t.app.view;
  for (const group of groups) {
    await expect(view.testId(`view-index-${group.id}-count`)).toHaveAttribute('data-count', String(Object.keys(demos[group.id]).length));
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

// The Chinese text of each group, whatever language the device is in.
const zhNames: Record<string, string[]> = {
  'view-text': ['段落', '输入时的建议', '尚未展示'],
  'view-gestures': ['指针的位置', '滑动行显示操作', '尚未展示'],
  'view-overlays': ['在触发处旁边打开的浮层', '尚未展示'],
};
for (const group of groups) {
  spec(`${group.page} reads in Chinese when asked`, { covers: ['view:catalogue'], start: { page: group.page, query: { lang: 'zh' } } }, async (t) => {
    const view = t.app.view;
    const names = zhNames[group.page];
    const missing = names[names.length - 1];
    for (const title of names.slice(0, -1)) expect(await view.getByRole('heading', { name: title }).count()).toBe(1);
    await expect(view.testId('view-group-missing')).toContainText(missing);
  });
}

spec('a group page reads in English when asked', { covers: ['view:catalogue'], start: { page: 'view-text', query: { lang: 'en' } } }, async (t) => {
  const view = t.app.view;
  expect(await view.getByRole('heading', { name: 'Suggestions while typing' }).count()).toBe(1);
  await expect(view.testId('view-group-missing')).toContainText('Not shown yet');
});
