import { expect, spec } from '@lingxia/test';
import type { DesktopWindowInfo } from '@lingxia/types/automation';
import { SHOWCASE_APP_ID } from '../../helpers/app.js';
import { runtimePlatform } from '../../helpers/platform.js';
import { bindFixture, eventually } from '../../helpers/poll.js';

const CHAT_APP_ID = 'lingxia-chat';

interface BannerProbe {
  result: unknown;
}
type Probes = Record<string, BannerProbe | undefined>;

/**
 * A guest lxapp (Chat, docked as an aside) speaks only for itself through
 * `lx.host.notification` and `lx.host.banner`: a notification may only open
 * Chat, the Control app cannot dismiss Chat's banner by naming the same id,
 * and Chat may come forward with `lx.surface.bringToFront` only after the
 * person picks an action on its banner.
 *
 * Picking an action needs the card's accessibility node, which only the macOS
 * card has; on Windows the card is drawn by hand, so that phase runs on macOS.
 */
spec('a guest lxapp gets attention for itself only', {
  id: 'DESKTOP-GUEST-ATTENTION-001',
  covers: ['lx.surface.bringToFront'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app, namespace, defer } = bindFixture(t, 'DESKTOP-GUEST-ATTENTION-001');
  const platform = await runtimePlatform(app);
  const desktop = t.automation.desktop;
  const bannerId = `guest-${namespace}`.slice(0, 64);

  defer(async () => {
    await app.logic.eval(async ({ lx }, id) => {
      await lx.host.banner?.dismiss(id).catch(() => undefined);
    }, bannerId).catch(() => undefined);
    const manager = t.automation.lxapps;
    const chat = (await manager.list()).find((candidate) => candidate.appId === CHAT_APP_ID);
    if (chat && chat.status !== 'closed') await manager.close({ app: CHAT_APP_ID }).catch(() => undefined);
  });

  await app.logic.eval({ timeout: 20_000 }, async ({ lx }) => {
    await lx.shell.openApp('lingxia-chat', { as: 'aside', edge: 'right' });
  });
  const chat = t.automation.lxapp(CHAT_APP_ID);

  const offered = await chat.logic.eval(({ lx }) => ({
    banner: typeof lx.host.banner?.show,
    supportsBanner: lx.supports('app.banner'),
    notification: typeof lx.host.notification?.show,
    bringToFront: typeof lx.surface.bringToFront,
  }));
  expect(offered).toEqual({
    banner: 'function',
    supportsBanner: true,
    notification: 'function',
    bringToFront: 'function',
  });

  // Every refusal lands before anything is posted, so none of these prompts.
  const refused = await chat.logic.eval({ timeout: 20_000 }, async ({ lx }) => {
    const show = lx.host.notification!.show as (options: unknown) => Promise<unknown>;
    const code = async (target: unknown) => {
      try {
        await show({ id: 'guest-scope', title: 'scope', target });
        return 'posted';
      } catch (error) {
        return (error as { code?: string }).code ?? String(error);
      }
    };
    let front = 'resolved';
    try {
      await lx.surface.bringToFront({ query: { probe: 'no-activation' } });
    } catch (error) {
      front = (error as { code?: string }).code ?? String(error);
    }
    return {
      activate: await code({ kind: 'activate' }),
      page: await code({ kind: 'page', page: 'chat' }),
      route: await code({ kind: 'route', name: 'anything' }),
      appLink: await code({ kind: 'appLink', url: 'https://example.com/x' }),
      otherApp: await code({ kind: 'app', appId: 'lingxia-showcase' }),
      bringToFront: front,
    };
  });
  expect(refused).toEqual({
    activate: 'E_PERMISSION_DENIED',
    page: 'E_PERMISSION_DENIED',
    route: 'E_PERMISSION_DENIED',
    appLink: 'E_PERMISSION_DENIED',
    otherApp: 'E_PERMISSION_DENIED',
    bringToFront: 'E_PERMISSION_DENIED',
  });

  // Its own notification, opening itself, under its own id.
  const own = await chat.logic.eval({ timeout: 20_000 }, async ({ lx }) => {
    const n = lx.host.notification!;
    const permission = await n.getPermission();
    try {
      const shown = await n.show({
        id: 'guest-own',
        title: 'Run finished',
        body: 'DESKTOP-GUEST-ATTENTION-001',
        target: { kind: 'app', appId: 'lingxia-chat', query: { run: 'probe' } },
        silent: true,
      });
      await n.cancel('guest-own');
      await n.cancelAll();
      return { permission, id: shown.id, status: shown.status as string, error: null };
    } catch (error) {
      return { permission, id: null, status: null, error: (error as { code?: string }).code ?? String(error) };
    }
  });
  if (own.permission === 'granted' || own.error === null) {
    expect({ id: own.id, posted: own.status === 'posted' || own.status === 'suppressed' })
      .toEqual({ id: 'guest-own', posted: true });
  }

  const appWindows = await t.automation.lxapps.windows();
  const hostWindowId = (appWindows.find((window) => window.main) ?? appWindows[0])?.id;
  const hostPid = (await desktop.windows()).find((window) => window.id === hostWindowId)?.pid;
  const before = new Set((await desktop.windows()).filter((window) => window.visible).map((window) => window.id));

  await chat.logic.eval(({ lx }, key, id) => {
    const probes = globalThis as unknown as Probes;
    const probe: BannerProbe = { result: null };
    probes[key] = probe;
    void lx.host.banner!.show({
      id,
      title: 'Run finished',
      body: 'DESKTOP-GUEST-ATTENTION-001',
      actions: [{ id: 'open', label: 'Open run', style: 'primary' }],
      timeoutMs: 0,
    }).then(
      (result) => { probe.result = result; },
      (error: { code?: string }) => { probe.result = { error: error.code ?? String(error) }; },
    );
    return true;
  }, namespace, bannerId);
  const bannerResult = () => chat.logic.eval((_, key) => (
    (globalThis as unknown as Probes)[key]?.result ?? null
  ), namespace);

  // The Control app's ids are not the guest's: the same id names nothing.
  await app.logic.eval(async ({ lx }, id) => { await lx.host.banner!.dismiss(id); }, bannerId);
  await new Promise<void>((resolve) => setTimeout(resolve, 400));
  expect(await bannerResult()).toBe(null);

  if (platform === 'macos') {
    const card = await eventually(
      async () => (await desktop.windows()).find((window: DesktopWindowInfo) => (
        window.pid === hostPid && window.visible && !before.has(window.id)
      )),
      (window) => window !== undefined,
      { describe: 'the guest banner on screen', timeoutMs: 5_000 },
    );
    await desktop.ax.invoke({ window: card!.id, match: 'name:Open run' });
    expect(await eventually(bannerResult, (value) => value !== null, { describe: 'the banner action' }))
      .toEqual({ status: 'ok', id: bannerId, action: 'open' });

    await chat.logic.eval({ timeout: 20_000 }, async ({ lx }) => {
      await lx.surface.bringToFront({ query: { probe: 'after-action' } });
    });
    // The activation is spent by that call.
    const again = await chat.logic.eval({ timeout: 20_000 }, async ({ lx }) => {
      try {
        await lx.surface.bringToFront();
        return 'resolved';
      } catch (error) {
        return (error as { code?: string }).code ?? String(error);
      }
    });
    expect(again).toBe('E_PERMISSION_DENIED');
  } else {
    await chat.logic.eval(async ({ lx }, id) => { await lx.host.banner!.dismiss(id); }, bannerId);
    expect(await eventually(bannerResult, (value) => value !== null, { describe: 'the guest dismissal' }))
      .toEqual({ status: 'canceled', id: bannerId, reason: 'dismissed' });
  }

  // The Control app needs no activation, and the query reaches App onShow.
  await app.logic.eval({ timeout: 20_000 }, async ({ lx }, probe) => {
    await lx.surface.bringToFront({ query: { probe } });
  }, namespace);
  const shown = await eventually(
    () => app.logic.eval(({ getApp }) => {
      const instance = getApp() as unknown as {
        globalData: { lifecycle: { lastShow?: { reason?: string; query?: Record<string, unknown> } } };
      } | null;
      const last = instance?.globalData.lifecycle.lastShow;
      return { reason: last?.reason ?? null, probe: last?.query?.probe ?? null };
    }),
    (value) => value.probe === namespace,
    { describe: 'the Control app reopen query in App onShow' },
  );
  expect(shown).toEqual({ reason: 'switch_back', probe: namespace });
});
