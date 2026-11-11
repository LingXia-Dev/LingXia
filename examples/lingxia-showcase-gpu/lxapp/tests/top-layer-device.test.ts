import { expect, spec } from '@lingxia/test';
import { harmonyDevice, type HarmonyNode } from '@lingxia/test/showcase/helpers/harmony-device.js';

const centre = ({ bounds: [l, t, r, b] }: HarmonyNode): [number, number] => [Math.round((l + r) / 2), Math.round((t + b) / 2)];
const sum = (rgb: number[]) => rgb[0] + rgb[1] + rgb[2];

spec.configure({ requires: { args: ['harmonyDevice'] } });

// A page dialog sits in the page's top layer, and its backdrop covers the host's bars as well as the
// page: under it a bar point dims as much as a page point. Read from the composited screen, since
// the bars are host views, not part of the GPU page.
spec('a page dialog dims the nav and tab bars as it dims the page', {
  id: 'TOP-LAYER-BARS-001', covers: ['view:dialog'],
  reason: 'reads the composited screen through the Harmony device fixture',
  start: { page: 'view-lab' },
}, async (t) => {
  const view = t.app.view;
  const device = harmonyDevice(t);
  await view.testId('lab-tab-feedback').click();
  const layout = await device.nodes();
  const one = (labels: string[], pick: (nodes: HarmonyNode[]) => HarmonyNode | undefined) => {
    const node = pick(layout.filter(node => labels.includes(node.text)));
    if (!node) throw new Error(`none of ${labels.join(', ')} on screen`);
    return node;
  };
  // The nav title is above the tab label of the same page.
  const title = one(['View lab', '视图实验'], nodes => nodes.sort((a, b) => a.bounds[1] - b.bounds[1])[0]);
  const tab = one(['View Lab', '实验室'], nodes => nodes.sort((a, b) => b.bounds[1] - a.bounds[1])[0]);
  const width = Math.max(...layout.map(node => node.bounds[2]));
  // Bar backgrounds beside their labels, and an empty page point below the centred dialog.
  const nav: [number, number] = [Math.round(width * 0.1), centre(title)[1]];
  const bar: [number, number] = [Math.round(width * 0.02), centre(tab)[1]];
  const page: [number, number] = [Math.round(width * 0.9), Math.round(centre(title)[1] + (centre(tab)[1] - centre(title)[1]) * 0.8)];
  const points = [nav, bar, page];
  const before = await device.pixels(points);
  for (const rgb of before) expect(sum(rgb), `lit before the dialog: ${rgb}`).toBeGreaterThan(150);

  await view.testId('lab-overlay-confirm').click();
  const dialog = view.getByRole('dialog', { name: 'Delete note?' });
  await expect(dialog).toBeVisible();
  await new Promise<void>(resolve => setTimeout(() => resolve(), 500));
  const after = await device.pixels(points);
  const ratio = (i: number) => sum(after[i]) / sum(before[i]);
  expect(ratio(2), `the backdrop dims the page: ${before} -> ${after}`).toBeLessThan(0.9);
  expect(Math.abs(ratio(0) - ratio(2)), `the nav bar dims as the page does: ${before[0]} -> ${after[0]}`).toBeLessThan(0.1);
  expect(Math.abs(ratio(1) - ratio(2)), `the tab bar dims as the page does: ${before[1]} -> ${after[1]}`).toBeLessThan(0.1);
  await view.testId('lab-dialog-cancel').click();
  await expect(dialog).not.toBeAttached();
});

// The system back gesture is a close request to the topmost dialog, as Escape is; the page stays.
spec('the system back gesture closes an open dialog and leaves the page where it was', {
  id: 'TOP-LAYER-BACK-001', covers: ['view:dialog'],
  reason: 'performs the system back gesture through the Harmony device fixture',
  start: { page: 'view-lab' },
}, async (t) => {
  const view = t.app.view;
  const device = harmonyDevice(t);
  await view.testId('lab-tab-feedback').click();
  const last = await view.testId('lab-overlay-last').textContent();
  for (const [open, name] of [['lab-overlay-confirm', 'Delete note?'], ['lab-overlay-sheet', 'Filters']] as const) {
    await view.testId(open).click();
    const dialog = view.getByRole('dialog', { name });
    await expect(dialog).toBeVisible();
    await device.edgeBack();
    await expect(dialog).not.toBeAttached();
    expect((await t.app.nav.current()).name, `back after "${name}" stays on the page`).toBe('view-lab');
    // Back cancels without choosing an action.
    await expect(view.testId('lab-overlay-last')).toHaveText(last ?? '');
  }
});

// Full screen covers the host's bars, and a finger on their strip reaches the page, never a tab
// item under it: a leaked tap would switch tabs or open More.
spec('a finger on the covered tab bar strip in full screen stays with the video', {
  id: 'TOP-LAYER-BARS-002', covers: ['view:dialog'],
  reason: 'taps through the system input path of the Harmony device fixture',
  start: { page: 'view-lab' },
}, async (t) => {
  const view = t.app.view;
  const device = harmonyDevice(t);
  await view.testId('lab-tab-media').click();
  await view.testId('lab-video-controls-fullscreen').click();
  const layer = view.getByRole('dialog', { name: 'Full screen video' });
  await expect(layer).toBeVisible();
  // A landscape picture turns the window.
  await expect.poll(async () => { const area = await view.viewport(); return area.width > area.height; }, { timeout: 5_000 }).toBe(true);
  await new Promise<void>(resolve => setTimeout(() => resolve(), 500));
  // The tab items still lie under the layer: a finger on each label, More included.
  const labels = ['Home', '首页', 'API', '接口', 'Motion', '动效', 'More', '更多'];
  const items = (await device.nodes()).filter(node => labels.includes(node.text));
  expect(items.length, 'the covered tab labels').toBe(4);
  for (const item of items) {
    await device.click(...centre(item));
    await new Promise<void>(resolve => setTimeout(() => resolve(), 400));
  }
  expect((await t.app.nav.current()).name).toBe('view-lab');
  await expect(layer).toBeVisible();
  // More's panel lists the tabs past the bar.
  expect((await device.nodes()).some(node => ['ToDo', '待办'].includes(node.text)), 'More stays closed').toBe(false);
  await view.testId('lx-fullscreen-controls-fullscreen').click();
  await expect(layer).not.toBeAttached();
  await expect.poll(async () => { const area = await view.viewport(); return area.height > area.width; }, { timeout: 5_000 }).toBe(true);
});
