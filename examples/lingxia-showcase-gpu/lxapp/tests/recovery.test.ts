import { expect, spec, type Fixture } from '@lingxia/test';
import { openDemo } from './open-demo';

// The host draws the recovery panel in the page's display language.
const words = {
  failed: ["This page couldn't be displayed.", '暂时无法显示此页面。'],
  retry: ['Retry', '重试'],
  back: ['Go back', '返回'],
  slow: ['This page is taking longer to load. You can go back and try again.', '页面加载时间较长，你可以返回后重试。'],
  // A held first state ends in a Logic restart offer.
  restart: ['Retry', '重试', 'Reload page', '重新载入页面'],
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

spec('a pushed page whose View throws shows the recovery panel; Retry fails again and keeps the page', {
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

spec('Go back on the recovery panel leaves only the failed page', {
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

// `devHoldFirstState`: under lingxia dev the page's Logic never sends its first state.
spec('a page whose first state never comes shows the slow-load notice; Retry waits again and Go back leaves only that page', {
  covers: ['view:recovery'], start: openDemo('recovery'), requires: { chrome: true },
}, async (t) => {
  const before = await openFailing(t, 'lab-recovery-stall', 'spec-stall');
  let elements = await slowPanel(t);
  labelOf(elements, words.back);
  await t.app.chrome.tap(labelOf(elements, words.restart));
  // The first state is held again: the notice returns, and the page stays on the stack.
  elements = await slowPanel(t);
  expect((await t.app.nav.stack()).length).toBe(before.length + 1);
  await expect(t.app.view.testId('spec-stall-ready')).not.toBeAttached();
  await t.app.chrome.tap(labelOf(elements, words.back));
  await t.waitFor(() => t.app.nav.stack(), { until: (stack) => stack.length === before.length });
  expect((await t.app.nav.stack()).map((page) => page.path)).toEqual(before.map((page) => page.path));
  await expect(t.app.view.testId('lab-recovery-stall')).toBeVisible();
});
