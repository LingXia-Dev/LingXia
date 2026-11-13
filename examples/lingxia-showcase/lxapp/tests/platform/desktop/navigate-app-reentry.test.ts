import { expect, test } from '@lingxia/test';
import type { TestApp } from '@lingxia/test';
import type { LxAppRuntimeInfo } from '@lingxia/types/automation';
import { SHOWCASE_APP_ID } from '../../helpers/app.js';
import { bindFixture, eventually } from '../../helpers/poll.js';

const CHAT_APP_ID = 'lingxia-chat';

const platform = (globalThis.__LINGXIA_AUTOMATION_HOST__?.args ?? {} as Record<string, string>).platform?.toLowerCase();
test.configure({ skip: platform === 'windows' || platform === 'macos' ? undefined : 'macOS and Windows only' });

const isOpen = (list: LxAppRuntimeInfo[], appid: string): boolean => (
  list.some((row) => row.appId === appid && row.status === 'opened')
);

test('navigateToApp presents an open lxapp again after the caller came back to the front', {
  id: 'NAV-APP-DESKTOP-002',
  covers: ['lx.navigateToApp', 'LxAppManager.current'],
  app: SHOWCASE_APP_ID,
  timeout: 90_000,
}, async (t) => {
  const { app, defer } = bindFixture(t, 'NAV-APP-DESKTOP-002');
  const manager = t.automation.lxapps;

  const inAsideSlot = async (): Promise<boolean> => (
    (await app.surfaceLayout()).asideSlots.some((slot) => slot.children.includes(CHAT_APP_ID))
  );
  // Chat is also the host's declared right aside. Once an earlier spec opened
  // that slot it outlives the lxapp, and navigateToApp would reopen chat into
  // the aside instead of the main this spec walks.
  const closeChat = async (): Promise<void> => {
    if (await inAsideSlot()) {
      await app.logic.eval({ timeout: 20_000 }, async ({ lx }, id) => {
        await (await lx.surface.openDeclared(id)).close();
      }, CHAT_APP_ID);
    }
    if (isOpen(await manager.list(), CHAT_APP_ID)) {
      await manager.close({ app: CHAT_APP_ID }).catch(() => undefined);
    }
    await eventually(
      async () => ({ open: isOpen(await manager.list(), CHAT_APP_ID), aside: await inAsideSlot() }),
      ({ open, aside }) => !open && !aside,
      { describe: 'chat and its aside slot to close' },
    );
  };
  await closeChat();
  defer(closeChat);

  // The promise can settle after the main has switched away from the caller,
  // so fire it and watch the host instead; a rejection still fails the hop.
  const hop = async (from: TestApp, appId: string): Promise<void> => {
    await from.logic.eval({ timeout: 20_000 }, ({ lx }, target) => {
      const key = '__navigateAppReentryError';
      delete (globalThis as Record<string, unknown>)[key];
      lx.navigateToApp({ appId: target }).catch((error: { code?: string } | null) => {
        (globalThis as Record<string, unknown>)[key] = error?.code ?? String(error);
      });
      return 'scheduled';
    }, appId);
    const shown = await eventually(
      async () => ({
        main: (await app.surfaceLayout()).activeMainId,
        current: (await manager.current()).appId,
      }),
      ({ main, current }) => main === appId && current === appId,
      { describe: `${appId} to be the visible main and the current lxapp`, timeoutMs: 20_000 },
    );
    expect(shown).toEqual({ main: appId, current: appId });
  };

  // Home's App onShow/onHide counts: each hop must reach the app it leaves
  // or enters, not only change what is on screen.
  const homeLifecycle = (): Promise<{ shows: number; hides: number }> => (
    app.logic.eval(({ getApp }) => {
      const lifecycle = (getApp() as unknown as {
        globalData?: { lifecycle?: { shows?: number; hides?: number } };
      } | null)?.globalData?.lifecycle;
      return { shows: lifecycle?.shows ?? 0, hides: lifecycle?.hides ?? 0 };
    })
  );
  const expectHomeEvent = async (key: 'shows' | 'hides', before: number, step: string) => {
    await eventually(homeLifecycle, (counts) => counts[key] > before, {
      describe: `home App ${key === 'shows' ? 'onShow' : 'onHide'} after ${step}`,
    });
  };

  await t.step('home opens chat', async () => {
    const before = await homeLifecycle();
    await hop(app, CHAT_APP_ID);
    await expectHomeEvent('hides', before.hides, 'opening chat');
  });
  await t.step('chat brings home back to the front', async () => {
    const chat = t.automation.lxapp(CHAT_APP_ID);
    await eventually(() => chat.logic.eval(() => true), (ready) => ready === true, {
      describe: 'chat Logic runtime to answer',
      retryIf: () => true,
    });
    const before = await homeLifecycle();
    await hop(chat, SHOWCASE_APP_ID);
    await expectHomeEvent('shows', before.shows, 'chat brought it back');
  });
  // The regression: chat stayed on top of the lxapp stack while home was on
  // screen, so this resolved without presenting anything.
  await t.step('home opens the still-open chat again', async () => {
    const before = await homeLifecycle();
    await hop(app, CHAT_APP_ID);
    await expectHomeEvent('hides', before.hides, 'opening chat again');
  });
});
