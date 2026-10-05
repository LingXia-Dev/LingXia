import type { TestApp, TestView } from '@lingxia/test';
import { expect, spec } from '@lingxia/test';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { waitForCurrentPage, waitForElementText } from '../helpers/page.js';
import { bindFixture, eventually } from '../helpers/poll.js';

// The leave guard (#540): a page with unsaved changes holds itself against
// *user* back. `nav.back({ user: true })` takes the navigation bar back
// button's path, guard included; a plain `nav.back()` is the programmatic pop
// the page itself answers with, and is never held.

async function openLeaveGuard(app: TestApp): Promise<TestView> {
  await app.nav.relaunch({ page: 'api' });
  await app.nav.to({ page: 'leaveGuard' });
  const view = (await app.page({ name: 'leaveGuard' }, { timeout: 30_000 })).view;
  await view.testId('leave-status').waitFor({ state: 'visible', timeout: 30_000 });
  return view;
}

async function makeDirty(t: Parameters<typeof waitForElementText>[0], view: TestView): Promise<void> {
  await view.testId('leave-draft').fill('unsaved draft');
  await waitForElementText(t, view, '[data-testid="leave-status"]', (text) => text === 'Unsaved changes');
}

spec('a guarded page asks on user back, and leaves only once the user discards', {
  id: 'UI-LEAVE-GUARD-001',
  app: SHOWCASE_APP_ID,
  timeout: 90_000,
}, async (t) => {
  const { app } = bindFixture(t, 'UI-LEAVE-GUARD-001');

  await t.step('a clean page leaves on user back', async () => {
    await openLeaveGuard(app);
    const landed = await app.nav.back({ user: true });
    expect(landed.name).toBe('api');
  });

  const view = await t.step('edit the draft', async () => {
    const view = await openLeaveGuard(app);
    await makeDirty(t, view);
    return view;
  });

  await t.step('user back stays and asks', async () => {
    const landed = await app.nav.back({ user: true });
    expect(landed.name).toBe('leaveGuard');
    await view.testId('leave-confirm').waitFor({ state: 'visible', timeout: 10_000 });
    await waitForElementText(t, view, '[data-testid="leave-requests"]', (text) => text.endsWith(': 1'));
  });

  await t.step('keep editing keeps the page and the draft', async () => {
    await view.testId('leave-keep').click();
    await view.testId('leave-confirm').waitFor({ state: 'detached', timeout: 10_000 });
    expect((await app.nav.current()).name).toBe('leaveGuard');
    await waitForElementText(t, view, '[data-testid="leave-status"]', (text) => text === 'Unsaved changes');
  });

  await t.step('asking again, then discarding, leaves the page', async () => {
    await app.nav.back({ user: true });
    await view.testId('leave-confirm').waitFor({ state: 'visible', timeout: 10_000 });
    await waitForElementText(t, view, '[data-testid="leave-requests"]', (text) => text.endsWith(': 2'));
    await view.testId('leave-discard').click();
    const back = await waitForCurrentPage(app, 'api', 30_000);
    expect(back.name).toBe('api');
  });
});

spec('saving releases the guard', {
  id: 'UI-LEAVE-GUARD-002',
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
}, async (t) => {
  const { app } = bindFixture(t, 'UI-LEAVE-GUARD-002');
  const view = await openLeaveGuard(app);
  await makeDirty(t, view);
  await view.testId('leave-save').click();
  await waitForElementText(t, view, '[data-testid="leave-status"]', (text) => text === 'Saved');

  const landed = await app.nav.back({ user: true });
  expect(landed.name).toBe('api');
});

spec('a programmatic back is never held', {
  id: 'UI-LEAVE-GUARD-003',
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
}, async (t) => {
  const { app } = bindFixture(t, 'UI-LEAVE-GUARD-003');
  const view = await openLeaveGuard(app);
  await makeDirty(t, view);

  // The page's own answer to a back request is this pop; it must not ask.
  const landed = await app.nav.back();
  expect(landed.name).toBe('api');
});

spec('tab labels and titles declared per language follow the display language', {
  id: 'UI-LOCALIZED-CHROME-001',
  covers: ['lx.host.control.displayLanguage.setPreference'],
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
}, async (t) => {
  const { app } = bindFixture(t, 'UI-LOCALIZED-CHROME-001');
  // lxapp.json: the ToDo tab's text and leaveGuard's navigationBar.title are
  // language maps ({ "en-US": …, "zh-CN": … }).
  const todoLabel = async () => (await app.info()).tabBar?.items.find((item) => item.index === 3)?.text ?? null;
  const title = async () => (await app.info()).navigationBar?.title ?? null;

  await openLeaveGuard(app);
  const preference = await app.logic.eval(({ lx }) => lx.host.control!.displayLanguage.getPreference());
  try {
    for (const [language, todo, heading] of [
      ['zh-CN', '待办', '离开确认'],
      ['en-US', 'ToDo', 'Leave guard'],
    ] as const) {
      await app.logic.eval(async ({ lx }, language) => {
        await lx.host.control!.displayLanguage.setPreference(language);
      }, language);
      await eventually(todoLabel, (text) => text === todo, {
        describe: `the ToDo tab label in ${language}`,
        timeoutMs: 10_000,
      });
      await eventually(title, (text) => text === heading, {
        describe: `the leaveGuard title in ${language}`,
        timeoutMs: 10_000,
      });
    }
  } finally {
    await app.logic.eval(async ({ lx }, preference) => {
      await lx.host.control!.displayLanguage.setPreference(preference);
    }, preference);
  }
});
