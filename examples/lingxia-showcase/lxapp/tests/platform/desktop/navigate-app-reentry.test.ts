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

  const closeChat = async (): Promise<void> => {
    if (isOpen(await manager.list(), CHAT_APP_ID)) {
      await manager.close({ app: CHAT_APP_ID }).catch(() => undefined);
    }
    await eventually(() => manager.list(), (list) => !isOpen(list, CHAT_APP_ID), {
      describe: 'chat to close',
    });
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

  await t.step('home opens chat', () => hop(app, CHAT_APP_ID));
  await t.step('chat brings home back to the front', async () => {
    const chat = t.automation.lxapp(CHAT_APP_ID);
    await eventually(() => chat.logic.eval(() => true), (ready) => ready === true, {
      describe: 'chat Logic runtime to answer',
      retryIf: () => true,
    });
    await hop(chat, SHOWCASE_APP_ID);
  });
  // The regression: chat stayed on top of the lxapp stack while home was on
  // screen, so this resolved without presenting anything.
  await t.step('home opens the still-open chat again', () => hop(app, CHAT_APP_ID));
});
