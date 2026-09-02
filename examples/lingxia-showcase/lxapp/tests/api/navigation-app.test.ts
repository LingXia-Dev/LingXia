import { expect, spec } from '@lingxia/test';
import type { LxAppRuntimeInfo } from 'lingxia-types/automation';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { waitForCurrentPageVisible } from '../helpers/page.js';
import { bindFixture, eventually } from '../helpers/poll.js';

const CHAT_APP_ID = 'lingxia-chat';

const isOpen = (list: LxAppRuntimeInfo[], appid: string): boolean => (
  list.some((row) => row.appid === appid && row.status === 'opened')
);

/** The host bundles lingxia-chat beside the Showcase, so a cross-lxapp hop needs no fixture. */
spec('hop to the bundled chat lxapp and back', {
  id: 'NAV-APP-001',
  covers: ['lx.navigateToApp', 'lx.navigateBackApp', 'LxAppManager.list', 'LxAppManager.current'],
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
}, async (t) => {
  const { app, namespace, defer } = bindFixture(t, 'NAV-APP-001');
  const manager = lx.automation().lxapps;
  const rows = (): Promise<LxAppRuntimeInfo[]> => manager.list();
  const currentApp = async (): Promise<string> => (await manager.current()).appid;

  await app.nav.relaunch({ page: 'home' });
  await waitForCurrentPageVisible(app, 'home', '[data-testid="home-page"]');
  defer(async () => {
    if (isOpen(await rows(), CHAT_APP_ID)) {
      await manager.close({ app: CHAT_APP_ID }).catch(() => undefined);
    }
    await eventually(currentApp, (appid) => appid === SHOWCASE_APP_ID, {
      describe: 'showcase to be current again after cleanup',
    });
  });

  // An earlier spec may have left chat parked; start from a closed target so
  // this proves an open, not a switch to something already running.
  if (isOpen(await rows(), CHAT_APP_ID)) {
    await manager.close({ app: CHAT_APP_ID });
    await eventually(rows, (list) => !isOpen(list, CHAT_APP_ID), { describe: 'parked chat to close first' });
  }
  const stateKey = `__lingxiaNavApp_${namespace.replace(/-/g, '_')}`;
  defer(async () => {
    await app.eval({ script: `delete globalThis[${JSON.stringify(stateKey)}]` }).catch(() => undefined);
  });

  await t.step('navigateToApp makes the target current and keeps the caller alive', async () => {
    // Fire without awaiting: the promise settles after the main switches away
    // from the caller, which is the transition this eval is part of. Keep the
    // rejection, so a failure names the cause instead of just timing out.
    await app.eval({
      script: `
        const state = { settled: null };
        globalThis[${JSON.stringify(stateKey)}] = state;
        lx.navigateToApp({ appId: ${JSON.stringify(CHAT_APP_ID)} })
          .then(() => { state.settled = { ok: true }; })
          .catch((error) => { state.settled = { ok: false, code: error && error.code, message: String(error && error.message) }; });
        return 'scheduled';
      `,
    });
    await eventually(
      async () => ({
        current: await currentApp(),
        settled: await app.eval({ script: `return globalThis[${JSON.stringify(stateKey)}]?.settled ?? null` }) as
          { ok: boolean; code?: string; message?: string } | null,
      }),
      ({ current, settled }) => current === CHAT_APP_ID || (settled !== null && !settled.ok),
      { describe: 'chat to become the current lxapp', timeoutMs: 20_000 },
    ).then(({ settled }) => {
      if (settled && !settled.ok) throw new Error(`navigateToApp rejected: ${settled.code} ${settled.message}`);
    });
    const list = await rows();
    expect(isOpen(list, CHAT_APP_ID)).toBe(true);
    // The caller is parked, not torn down.
    expect(isOpen(list, SHOWCASE_APP_ID)).toBe(true);
  });

  await t.step('navigateBackApp from the target returns to the caller and closes the target', async () => {
    const chat = lx.automation().lxapp(CHAT_APP_ID);
    await eventually(() => chat.eval({ script: 'return true', timeoutMs: 5_000 }), (ready) => ready === true, {
      describe: 'chat Logic runtime to answer',
      timeoutMs: 20_000,
      retryIf: () => true,
    });
    await chat.eval({ script: `void lx.navigateBackApp(); return 'scheduled';` });
    await eventually(currentApp, (appid) => appid === SHOWCASE_APP_ID, {
      describe: 'showcase to be current again after navigateBackApp',
      timeoutMs: 20_000,
    });
    await eventually(rows, (list) => !isOpen(list, CHAT_APP_ID), {
      describe: 'chat to close after navigateBackApp',
      timeoutMs: 10_000,
    });
    await waitForCurrentPageVisible(app, 'home', '[data-testid="home-page"]');
  });

  await t.step('navigateToApp rejects an unknown page name without leaving the caller', async () => {
    const rejected = await app.eval({
      script: `
        try {
          await lx.navigateToApp({ appId: ${JSON.stringify(CHAT_APP_ID)}, page: 'no-such-page' });
          return { ok: true };
        } catch (error) {
          return { ok: false, code: error && error.code };
        }
      `,
    }) as { ok: boolean; code?: string };
    expect(rejected.ok).toBe(false);
    expect(rejected.code).toBe('E_NOT_FOUND');
    expect(await currentApp()).toBe(SHOWCASE_APP_ID);
  });
});
