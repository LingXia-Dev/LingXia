import { expect, spec, type Fixture, type Locator } from '@lingxia/test';

// How a GPU page shares the bottom edge with the host tab bar: the page scroll runs under the bar
// and its end scrolls clear of it; an open dialog (sheet, picker, its scrim) lies over the bar.

type Box = { x: number; y: number; width: number; height: number };
const bottom = (box: Box) => box.y + box.height;

/** The page box and where its bottom bars begin, as the page's own `env.insets` measures them. */
async function bars(t: Fixture) {
  const lab = t.app.view.testId('view-lab');
  const page = await lab.boundingBox();
  const inset = Number(await lab.getAttribute('data-inset-bottom'));
  const safe = Number(await lab.getAttribute('data-safe-bottom'));
  // A desktop shell lists the tab items in its sidebar: nothing shares the bottom edge.
  if (inset <= safe) t.skip('no tab bar on the bottom edge');
  return { page, inset, top: bottom(page) - inset };
}

/** The tab items a user can reach, or null where the host does not expose its tab bar. */
async function tabItems(t: Fixture) {
  try {
    return await t.app.chrome.elements({ scope: 'tabBar' });
  } catch {
    return null;
  }
}

/** `dialog` reaches the page's bottom edge, and the host lifts the page over its tab bar until it closes. */
async function coversTheBar(t: Fixture, open: () => Promise<void>, dialog: Locator, close: () => Promise<void>) {
  const before = await tabItems(t);
  console.log(`tab items readable before: ${before?.length ?? 'unsupported'}`);
  const { page, inset } = await bars(t);
  await open();
  await expect(dialog).toBeVisible();
  // Once its entrance settles the panel reaches past the bar's top to the bottom edge (a detent may rest lower).
  let last = Number.NaN;
  const box = await t.waitFor(() => dialog.boundingBox(), {
    until: b => { const settled = b.y === last; last = b.y; return settled; }, interval: 100,
  });
  expect(bottom(box)).toBeGreaterThanOrEqual(bottom(page) - 1);
  expect(box.y).toBeLessThan(bottom(page) - inset);
  if (before?.length) await t.waitFor(() => tabItems(t), { until: items => items?.length === 0 });
  await close();
  await expect(dialog).not.toBeAttached();
  if (before?.length) await t.waitFor(() => tabItems(t), { until: items => items?.length === before.length });
}

spec('a tab page scroll runs under the tab bar and its end scrolls clear of it', { covers: ['view:scroll'], start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  const { page, inset, top } = await bars(t);
  const scroll = await view.testId('page-scroll').boundingBox();
  expect(Math.abs(bottom(scroll) - bottom(page))).toBeLessThanOrEqual(1);
  const last = view.testId('lab-transforms');
  let previous = Number.NaN;
  for (let attempt = 0; attempt < 8; attempt++) {
    await view.fling('up', 4000);
    const at = bottom(await last.boundingBox());
    if (at === previous) break;
    previous = at;
  }
  expect(previous).toBeLessThanOrEqual(top + 1);
  expect(previous).toBeGreaterThan(top - inset);
});

spec('another tab page scroll runs under the tab bar too', { covers: ['view:scroll'], start: { page: 'api' } }, async (t) => {
  const view = t.app.view;
  const page = await view.testId('api-page').boundingBox();
  const scroll = await view.testId('page-scroll').boundingBox();
  expect(Math.abs(bottom(scroll) - bottom(page))).toBeLessThanOrEqual(1);
});

// A fine pointer opens a popover, not a sheet with Cancel.
spec('a select picker covers the tab bar', { covers: ['view:select', 'view:dialog'], start: { page: 'view-lab' }, requires: { pointer: 'coarse' } }, async (t) => {
  const view = t.app.view;
  await view.testId('lab-tab-input').click();
  await coversTheBar(t, () => view.testId('lab-input-plan').click(), view.testId('lx-picker'),
    () => view.testId('lx-picker-cancel').click());
});

spec('a date picker covers the tab bar', { covers: ['view:input', 'view:dialog'], start: { page: 'view-lab' }, requires: { pointer: 'coarse' } }, async (t) => {
  const view = t.app.view;
  await view.testId('lab-tab-input').click();
  await coversTheBar(t, () => view.testId('lab-input-date').click(), view.testId('lx-picker'),
    () => view.testId('lx-picker-cancel').click());
});

spec('an overlay sheet covers the tab bar', { covers: ['view:dialog'], start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  await view.testId('lab-tab-feedback').click();
  await coversTheBar(t, () => view.testId('lab-overlay-sheet').click(), view.getByRole('dialog', { name: 'Filters' }),
    () => view.testId('lab-sheet-apply').click());
});

spec('a sheet with detents covers the tab bar', { covers: ['view:dialog', 'view:drag'], start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  await view.testId('lab-tab-gesture').click();
  await coversTheBar(t, () => view.testId('lab-detents-open').click(), view.testId('lab-detents-sheet'),
    () => view.testId('lab-detents-sheet').performAction('dismiss'));
});

spec('a tap at an element under the tab bar scrolls it clear of the bar first and lands on it', { covers: ['view:scroll'], start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  const { page, top } = await bars(t);
  await view.testId('lab-tab-swipe').click();
  const state = view.testId('lab-kit-pager-state');
  await expect(state).toHaveText('page=0');
  // Park a pager dot in the middle of the bar with a slow drag that does not fling.
  const dot = view.testId('lab-kit-pager-view-dot-2');
  const center = (box: Box) => box.y + box.height / 2;
  const dy = center(await dot.boundingBox()) - (top + bottom(page)) / 2;
  if (Math.abs(dy) > 1) await view.swipe({ from: [20, 400], to: [20, 400 - dy], durationMs: 600 });
  let last = Number.NaN;
  const parked = await t.waitFor(() => dot.boundingBox(), {
    until: b => { const settled = b.y === last; last = b.y; return settled; }, interval: 100,
  });
  expect(parked.y).toBeGreaterThanOrEqual(top);
  await expect(dot).not.toBeInViewport();
  await (await view.touch(dot)).up();
  await expect(state).toHaveText('page=2');
  expect(bottom(await dot.boundingBox())).toBeLessThanOrEqual(top);
});

spec('a tap at an element partly under the tab bar lands on its visible part without scrolling', { covers: ['view:scroll'], start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  const { top } = await bars(t);
  await view.testId('lab-tab-swipe').click();
  const state = view.testId('lab-kit-pager-state');
  await expect(state).toHaveText('page=0');
  // Park a pager dot so its centre sits just under the bar's top edge and its upper part shows.
  const dot = view.testId('lab-kit-pager-view-dot-2');
  const center = (box: Box) => box.y + box.height / 2;
  let parked = await dot.boundingBox();
  for (let attempt = 0; attempt < 8; attempt++) {
    const dy = center(parked) - (top + 1);
    if (Math.abs(dy) <= 1) break;
    // A drag first spends the touch slop: lengthen it by about that much. It starts midway above
    // the bar and stays clear of both edges, so a short landscape page takes it in steps.
    const from = top / 2;
    const to = Math.min(Math.max(from - dy - Math.sign(dy) * 8, 8), top - 8);
    await view.swipe({ from: [20, from], to: [20, to], durationMs: 600 });
    let last = Number.NaN;
    parked = await t.waitFor(() => dot.boundingBox(), {
      until: b => { const settled = b.y === last; last = b.y; return settled; }, interval: 100,
    });
  }
  expect(parked.y).toBeLessThan(top - 8);
  expect(center(parked)).toBeGreaterThan(top);
  await expect(dot).toBeInViewport();
  await (await view.touch(dot)).up();
  await expect(state).toHaveText('page=2');
  expect((await dot.boundingBox()).y).toBe(parked.y);
});

spec('a relaunch onto a page outside the tab bar hides the bar and keeps no room for it, and back onto a tab page shows it', { start: { page: 'view-lab' } }, async (t) => {
  const before = await tabItems(t);
  console.log(`tab items readable before: ${before?.length ?? 'unsupported'}`);
  await t.app.nav.relaunch({ page: 'motion-target' });
  const page = t.app.view.testId('motion-lab');
  await expect(page).toBeVisible();
  const inset = Number(await page.getAttribute('data-inset-bottom'));
  const safe = Number(await page.getAttribute('data-safe-bottom'));
  expect(inset, 'no room kept for a bar').toBe(safe);
  if (before?.length) await t.waitFor(() => tabItems(t), { until: items => items?.length === 0 });
  await t.app.nav.relaunch({ page: 'view-lab' });
  await bars(t);
  if (before?.length) await t.waitFor(() => tabItems(t), { until: items => items?.length === before.length });
});
