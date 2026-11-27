import { expect, test, type Fixture } from '@lingxia/test';
import { openDemo, openUnderTabs } from './open-demo';

// The host draws the recovery panel in the page's display language.
const words = {
  failed: ["This page couldn't be displayed.", '暂时无法显示此页面。'],
  retry: ['Retry', '重试'],
  back: ['Go back', '返回'],
  slow: ['This page is taking longer than usual to load.', '页面加载时间比平时长。'],
  wait: ['Keep waiting', '继续等待'],
  updateFailed: ["This page couldn't update. Your previous view is still available.", '页面更新失败，已保留上次显示的内容。'],
};

async function panel(t: Fixture) {
  return t.waitFor(() => t.app.chrome.elements(), {
    until: (elements) => elements.some((e) => words.failed.includes(e.label)),
    timeout: 8_000,
  });
}

const labelOf = (elements: { label: string }[], choices: string[]) => {
  const found = elements.find((e) => choices.includes(e.label));
  if (!found) throw new Error(`no ${choices.join(' / ')} on the recovery panel: ${elements.map((e) => e.label).join(', ')}`);
  return found.label;
};

async function openFailing(t: Fixture, button = 'lab-recovery-open', page = 'spec-fault') {
  const before = await t.app.nav.stack();
  await t.app.view.testId(button).click();
  await t.waitFor(() => t.app.nav.current(), { until: (current) => current.path.includes(page) });
  return before;
}

/** The slow-load notice over a page whose first state is held, with what it offers. */
async function slowPanel(t: Fixture) {
  return t.waitFor(() => t.app.chrome.elements(), {
    until: (elements) => elements.some((e) => words.slow.includes(e.label)),
    timeout: 12_000,
  });
}

test('a pushed page whose View throws shows the recovery panel; Retry fails again and keeps the page', {
  expectPageFailure: { page: /spec-fault/ },
  covers: ['view:recovery'], start: openDemo('recovery'), requires: { chrome: true },
}, async (t) => {
  const before = await openFailing(t);
  const elements = await panel(t);
  labelOf(elements, words.back);
  await t.app.chrome.tap(labelOf(elements, words.retry));
  // The View throws again: the panel returns, and the page stays on the stack.
  await panel(t);
  const failure = await t.app.view.testId('anything').boundingBox().then(() => '', (error: unknown) => String(error));
  expect(failure).toContain('spec-fault: this page fails on purpose');
  expect((await t.app.nav.stack()).length).toBe(before.length + 1);
});

test('Go back on the recovery panel leaves only the failed page', {
  expectPageFailure: { page: /spec-fault/ },
  covers: ['view:recovery'], start: openDemo('recovery'), requires: { chrome: true },
}, async (t) => {
  const before = await openFailing(t);
  const elements = await panel(t);
  await t.app.chrome.tap(labelOf(elements, words.back));
  await t.waitFor(() => t.app.nav.stack(), { until: (stack) => stack.length === before.length });
  const after = await t.app.nav.stack();
  expect(after.map((page) => page.path)).toEqual(before.map((page) => page.path));
  await expect(t.app.view.testId('lab-recovery-open')).toBeVisible();
});

test('the recovery buttons open their pages from the View tab as from the group page', {
  expectPageFailure: { page: /spec-fault/ },
  covers: ['view:recovery'], start: openUnderTabs('recovery'), requires: { chrome: true },
}, async (t) => {
  const before = await openFailing(t);
  await t.app.chrome.tap(labelOf(await panel(t), words.back));
  await t.waitFor(() => t.app.nav.stack(), { until: (stack) => stack.length === before.length });
  await expect(t.app.view.testId('lab-recovery-stall')).toBeVisible();
});

// `devHoldFirstState`: under lingxia dev the page's Logic never sends its first state.
test('a page whose first state never comes shows the slow-load notice; Keep waiting closes it until it returns, and Go back leaves only that page', {
  expectPageFailure: { page: /spec-stall/ },
  covers: ['view:recovery'], start: openDemo('recovery'), requires: { chrome: true },
}, async (t) => {
  const before = await openFailing(t, 'lab-recovery-stall', 'spec-stall');
  let elements = await slowPanel(t);
  labelOf(elements, words.back);
  await t.app.chrome.tap(labelOf(elements, words.wait));
  // The notice closes, then returns while the first state is still held; the page stays on the stack.
  await t.waitFor(() => t.app.chrome.elements(), { until: (now) => !now.some((e) => words.slow.includes(e.label)), timeout: 3_000 });
  elements = await slowPanel(t);
  // Read through the chrome only: a query on the held page answers that it failed to load.
  expect((await t.app.nav.stack()).length).toBe(before.length + 1);
  await t.app.chrome.tap(labelOf(elements, words.back));
  await t.waitFor(() => t.app.nav.stack(), { until: (stack) => stack.length === before.length });
  expect((await t.app.nav.stack()).map((page) => page.path)).toEqual(before.map((page) => page.path));
  await expect(t.app.view.testId('lab-recovery-stall')).toBeVisible();
});

// An update that fails over a shown page keeps the page: a banner, not a panel over it. The host
// draws it in each appearance, so the spec reads it in both.
test('an update that fails after the page has shown keeps the page under a banner with Retry and Go back, light and dark', {
  expectPageFailure: { page: /spec-break/ },
  covers: ['view:recovery'], start: openDemo('recovery'), requires: { chrome: true },
}, async (t) => {
  const preference = await t.app.logic.eval(({ lx }) => lx.host.appearancePreference.get() ?? null);
  if (preference === null) t.skip('this host does not let an lxapp set its appearance; check the banner by hand in light and dark');
  t.defer(() => t.app.logic.eval(({ lx }, look) => lx.host.appearancePreference.set(look), preference!));
  const banner = async () => {
    const elements = await t.waitFor(() => t.app.chrome.elements(), {
      // Right after Retry the panel can show its message a moment before it offers anything.
      until: (now) => [words.updateFailed, words.retry, words.back]
        .every((choices) => now.some((e) => choices.includes(e.label))),
      timeout: 8_000,
    });
    return { elements, notice: elements.find((e) => words.updateFailed.includes(e.label))! };
  };
  for (const look of ['light', 'dark'] as const) {
    await t.app.logic.eval(({ lx }, look) => lx.host.appearancePreference.set(look), look);
    const before = await openFailing(t, 'lab-recovery-break', 'spec-break');
    await expect(t.app.view.testId('spec-break-shown')).toBeVisible();
    // In the frame chrome bounds are in: a desktop device frame scales the page's own points.
    const window = await t.app.chrome.viewport();
    // The tap's update is the one that throws, so the frame the click waits for never comes.
    const tapped = t.app.view.testId('spec-break-go').click().catch(() => undefined);
    let { elements, notice } = await banner();
    await tapped;
    const [, y, width, height] = notice.bounds;
    // On screen, and a strip rather than a cover: most of the kept page shows around it.
    expect(width, `${look}: banner width`).toBeGreaterThan(0);
    expect(y, `${look}: banner top`).toBeGreaterThanOrEqual(0);
    expect(y + height, `${look}: banner bottom`).toBeLessThanOrEqual(window.top + window.height + 1);
    expect(height, `${look}: banner height`).toBeLessThan(window.height / 2);
    labelOf(elements, words.back);
    await t.app.chrome.tap(labelOf(elements, words.retry));
    // The data still breaks the View: the banner returns over the same page.
    ({ elements } = await banner());
    expect((await t.app.nav.stack()).length).toBe(before.length + 1);
    await t.app.chrome.tap(labelOf(elements, words.back));
    await t.waitFor(() => t.app.nav.stack(), { until: (stack) => stack.length === before.length });
    expect((await t.app.nav.stack()).map((page) => page.path)).toEqual(before.map((page) => page.path));
    await expect(t.app.view.testId('lab-recovery-break')).toBeVisible();
  }
});
