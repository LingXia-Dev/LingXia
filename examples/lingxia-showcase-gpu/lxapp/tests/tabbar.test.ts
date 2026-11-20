import { expect, test, type Fixture, type Locator } from '@lingxia/test';
import { openDemo, openUnderTabs, rootOf, navTarget } from './open-demo';

// How a GPU page shares the bottom edge with the host tab bar: the page scroll runs under the bar
// and its end scrolls clear of it; an open dialog (sheet, picker, its scrim) lies over the bar.

type Box = { x: number; y: number; width: number; height: number };
const bottom = (box: Box) => box.y + box.height;

/** The page box and where its bottom bars begin, as the page's own `env.insets` measures them. */
async function bars(t: Fixture, root = 'view-index') {
  const lab = t.app.view.testId(root);
  await expect(lab).toBeVisible();
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
async function coversTheBar(t: Fixture, root: string, open: () => Promise<void>, dialog: Locator, close: () => Promise<void>) {
  const before = await tabItems(t);
  console.log(`tab items readable before: ${before?.length ?? 'unsupported'}`);
  const { page, inset } = await bars(t, root);
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

test('a tab page scroll runs under the tab bar and its end scrolls clear of it', { covers: ['view:scroll'], start: openUnderTabs('controls') }, async (t) => {
  const view = t.app.view;
  const { page, inset, top } = await bars(t, rootOf('controls'));
  const scroll = await view.testId('page-scroll').boundingBox();
  expect(Math.abs(bottom(scroll) - bottom(page))).toBeLessThanOrEqual(1);
  const last = view.testId('lab-fragments');
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

test('another tab page scroll runs under the tab bar too', { covers: ['view:scroll'], start: { page: 'api' } }, async (t) => {
  const view = t.app.view;
  const page = await view.testId('api-page').boundingBox();
  const scroll = await view.testId('page-scroll').boundingBox();
  expect(Math.abs(bottom(scroll) - bottom(page))).toBeLessThanOrEqual(1);
});

// A fine pointer opens a popover, not a sheet with Cancel.
test('a select picker covers the tab bar', { covers: ['view:select', 'view:dialog'], start: openUnderTabs('select'), requires: { pointer: 'coarse' } }, async (t) => {
  const view = t.app.view;
  await coversTheBar(t, rootOf('select'), () => view.testId('lab-input-plan').click(), view.testId('lx-picker'),
    () => view.testId('lx-picker-cancel').click());
});

test('a date picker covers the tab bar', { covers: ['view:input', 'view:dialog'], start: openUnderTabs('fields'), requires: { pointer: 'coarse' } }, async (t) => {
  const view = t.app.view;
  await coversTheBar(t, rootOf('fields'), () => view.testId('lab-input-date').click(), view.testId('lx-picker'),
    () => view.testId('lx-picker-cancel').click());
});

test('an overlay sheet covers the tab bar', { covers: ['view:dialog'], start: openUnderTabs('bottomSheet') }, async (t) => {
  const view = t.app.view;
  await coversTheBar(t, rootOf('bottomSheet'), () => view.testId('lab-overlay-sheet').click(), view.getByRole('dialog', { name: 'Filters' }),
    () => view.testId('lab-sheet-apply').click());
});

test('a sheet with detents covers the tab bar', { covers: ['view:dialog', 'view:drag'], start: openUnderTabs('sheet') }, async (t) => {
  const view = t.app.view;
  await coversTheBar(t, rootOf('sheet'), () => view.testId('lab-detents-open').click(), view.testId('lab-detents-sheet'),
    () => view.testId('lab-detents-sheet').performAction('dismiss'));
});

test('a tap at an element under the tab bar scrolls it clear of the bar first and lands on it', { covers: ['view:scroll'], start: openUnderTabs('dots') }, async (t) => {
  const view = t.app.view;
  const { page, top } = await bars(t, rootOf('dots'));
  const state = view.testId('lab-kit-pager-state');
  await expect(state).toHaveAttribute('data-state', 'page=0');
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
  await expect(state).toHaveAttribute('data-state', 'page=2');
  expect(bottom(await dot.boundingBox())).toBeLessThanOrEqual(top);
});

test('a tap at an element partly under the tab bar lands on its visible part without scrolling', { covers: ['view:scroll'], start: openUnderTabs('dots') }, async (t) => {
  const view = t.app.view;
  const { top } = await bars(t, rootOf('dots'));
  const state = view.testId('lab-kit-pager-state');
  await expect(state).toHaveAttribute('data-state', 'page=0');
  // Park a pager dot so its centre sits just under the bar's top edge and its upper part shows.
  const dot = view.testId('lab-kit-pager-view-dot-2');
  const center = (box: Box) => box.y + box.height / 2;
  let parked = await dot.boundingBox();
  // A touch drag first spends the slop; a trackpad scroll has none. Learn it from each drag.
  let slop = 8;
  for (let attempt = 0; attempt < 8; attempt++) {
    const dy = center(parked) - (top + 1);
    if (Math.abs(dy) <= 1) break;
    // It starts midway above the bar and stays clear of both edges, so a short landscape page
    // takes it in steps.
    const from = top / 2;
    const to = Math.min(Math.max(from - dy - Math.sign(dy) * slop, 8), top - 8);
    await view.swipe({ from: [20, from], to: [20, to], durationMs: 600 });
    let last = Number.NaN;
    const before = parked.y;
    parked = await t.waitFor(() => dot.boundingBox(), {
      until: b => { const settled = b.y === last; last = b.y; return settled; }, interval: 100,
    });
    slop = Math.min(Math.max(Math.abs(to - from) - Math.abs(before - parked.y), 0), 16);
  }
  expect(parked.y).toBeLessThan(top - 8);
  expect(center(parked)).toBeGreaterThan(top);
  await expect(dot).toBeInViewport();
  await (await view.touch(dot)).up();
  await expect(state).toHaveAttribute('data-state', 'page=2');
  expect((await dot.boundingBox()).y).toBe(parked.y);
});

test('a relaunch onto a page outside the tab bar hides the bar and keeps no room for it, and back onto a tab page shows it', { start: { page: 'view' } }, async (t) => {
  const before = await tabItems(t);
  console.log(`tab items readable before: ${before?.length ?? 'unsupported'}`);
  await t.app.nav.reset(navTarget(openDemo('press')));
  const page = t.app.view.testId(rootOf('press'));
  await expect(page).toBeVisible();
  const inset = Number(await page.getAttribute('data-inset-bottom'));
  const safe = Number(await page.getAttribute('data-safe-bottom'));
  expect(inset, 'no room kept for a bar').toBe(safe);
  if (before?.length) await t.waitFor(() => tabItems(t), { until: items => items?.length === 0 });
  await t.app.nav.reset({ page: 'view' });
  await bars(t);
  if (before?.length) await t.waitFor(() => tabItems(t), { until: items => items?.length === before.length });
});

// Every tab page: at scroll end its last control rests above the bar; content passes under only while scrolling.
// A fling takes about 3 s on the macOS Runner (under 0.1 s on a phone), two or more a tab.
test('every tab page ends its content above the tab bar', { covers: ['view:scroll'], start: { page: 'view' }, timeout: 90_000 }, async (t) => {
  const view = t.app.view;
  const { inset } = await bars(t);
  const controls = 'button, input, select, textarea, checkbox, switch, slider, [role="button"], [role="link"]';
  // The bar this platform shows: Surface is a desktop tab. Tabs are told apart by icon, whatever the language.
  const known: Record<string, [string, string]> = {
    home: ['home', 'home-page'], api: ['api', 'api-page'], lab: ['view', 'view-index'], perf: ['perf-lab', 'perf-lab'],
    todo: ['todo', 'todo-page'], surface: ['surface', 'surface-page'],
  };
  const shown = ((await t.app.info()).tabBar?.items ?? []).map((item) => {
    const icon = /tab-([a-z]+)\.svg$/.exec(item.iconPath ?? '')?.[1] ?? '';
    if (!known[icon]) throw new Error(`a tab this spec does not know: ${item.text} (${item.iconPath})`);
    return known[icon];
  });
  expect(shown.length, 'the tab bar lists its tabs').toBeGreaterThanOrEqual(5);
  for (const [name, root] of shown) {
    await t.app.nav.go({ tab: name });
    const page = view.testId(root);
    await expect(page).toBeVisible();
    const pageBox = await page.boundingBox();
    const top = bottom(pageBox) - inset;
    const last = view.css(controls).last();
    // Start in the page gutter so a nested scroll (perf-lab's list) does not take the fling.
    const start: [number, number] = [pageBox.x + 6, pageBox.y + pageBox.height * 0.6];
    let previous = Number.NaN;
    for (let attempt = 0; attempt < 10; attempt++) {
      await view.fling('up', 4000, { at: start });
      // A fling returns before its glide ends: read the box once it stops moving.
      let moving = Number.NaN;
      const at = bottom(await t.waitFor(() => last.boundingBox(), {
        until: box => { const settled = box.y === moving; moving = box.y; return settled; }, interval: 150,
      }));
      if (at === previous) break;
      previous = at;
    }
    expect(previous, `${name}: last control clear of the bar`).toBeLessThanOrEqual(top + 1);
  }
});
