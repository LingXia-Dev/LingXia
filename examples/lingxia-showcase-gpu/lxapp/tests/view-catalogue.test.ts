import { expect, test } from '@lingxia/test';
import { demos, groups, missing } from '../shared/view-lab/text';

// The catalogue of View: an index of groups, each a pushed page of demos in one shape.
test('the index lists each group with its count and opens it', { covers: ['view:catalogue'], start: { page: 'view' } }, async (t) => {
  const view = t.app.view;
  for (const group of groups) {
    await expect(view.testId(`view-index-${group.id}-count`)).toHaveAttribute('data-count', String(Object.keys(demos[group.id]).length));
  }
  await view.testId('view-index-text').click();
  await expect(view.testId('view-group-text')).toBeVisible();
  expect((await t.app.nav.current()).path).toContain('view-text');
});

for (const group of groups) {
  test(`${group.page} shows every demo of its group and says what is missing`, { covers: ['view:catalogue'], start: { page: group.page } }, async (t) => {
    const view = t.app.view;
    for (const demo of Object.values(demos[group.id])) expect(await view.testId(demo.testId).count()).toBe(1);
    expect(await view.testId('view-group-missing').count()).toBe(missing[group.id].length ? 1 : 0);
  });
}

// The Chinese text of each group, whatever language the device is in.
const zhNames: Record<string, string[]> = {
  'view-drawing': ['渐变填充', '边框与圆角'],
  'view-media': ['视频', '内嵌网页', '图表', '进度'],
  'view-controls': ['下拉选择', '传给所有子元素的值'],
  'view-layout': ['网格', '报告自身尺寸的方块'],
  'view-text': ['段落', '选择文字', '已停用的链接'],
  'view-input': ['输入时的建议', '输入框', '双滑块'],
  'view-gestures': ['在停靠点之间拖动', '滑动行显示操作', '下拉刷新'],
  'view-pointer': ['指针的位置', '行里的控件'],
  'view-overlays': ['对话框', '底部面板', '按需唤出的菜单', '菜单里的菜单', '保持打开的菜单', '关闭前先询问的浮层', '在触发处旁边打开的浮层'],
  'view-scrolling': ['跳到某一行', '吸顶的标题', '带圆点的翻页'],
  'view-motion': ['按压反馈', '重新排序', '展开与收起的面板'],
  'view-appearance': ['浅色与深色', '安全区与键盘'],
  'view-accessibility': ['名称与角色', '文件夹树', '有名称的列表', '日期网格'],
};
for (const group of groups) {
  test(`${group.page} reads in Chinese when asked`, { covers: ['view:catalogue'], start: { page: group.page, params: { lang: 'zh' } } }, async (t) => {
    const view = t.app.view;
    // Retrying: the page can draw a first frame before its `lang` reaches it.
    for (const title of zhNames[group.page]) await expect(view.getByRole('heading', { name: title })).toHaveCount(1);
    if (missing[group.id].length) await expect(view.testId('view-group-missing')).toContainText('尚未展示');
  });
}

test('a group page reads in English when asked', { covers: ['view:catalogue'], start: { page: 'view-input', params: { lang: 'en' } } }, async (t) => {
  const view = t.app.view;
  await expect(view.getByRole('heading', { name: 'Suggestions while typing' })).toHaveCount(1);
  await expect(view.testId('view-group-missing')).toContainText('Not shown yet');
});

test('a finger on an index row opens its group', { covers: ['view:catalogue'], start: { page: 'view' } }, async (t) => {
  const view = t.app.view;
  // A real touch at each row's centre, not a dispatch to the element: the hit test decides.
  // The first rows: on screen on any phone without scrolling.
  for (const id of ['drawing', 'media', 'controls'] as const) {
    const box = await view.testId(`view-index-${id}`).boundingBox();
    const at = [box.x + box.width / 2, box.y + box.height / 2] as const;
    await view.swipe({ from: at, to: at, durationMs: 60, hold: 0 });
    await expect(view.testId(`view-group-${id}`)).toBeVisible({ timeout: 3_000 });
    await t.app.nav.back();
    await expect(view.testId('view-index')).toBeVisible();
  }
});
