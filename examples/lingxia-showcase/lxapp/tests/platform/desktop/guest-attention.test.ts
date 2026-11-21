import { expect, test } from '@lingxia/test';
import { notDesktop } from './only.js';
import type { DesktopWindowInfo } from '@lingxia/types/automation';
import { SHOWCASE_APP_ID } from '@lingxia/test/showcase/helpers/app.js';
import { runtimePlatform } from '@lingxia/test/showcase/helpers/platform.js';
import { bindFixture, eventually } from '@lingxia/test/showcase/helpers/poll.js';

test.configure({ skip: notDesktop });

const CHAT_APP_ID = 'lingxia-showcase-web';

interface BannerProbe {
  result: unknown;
}
type Probes = Record<string, BannerProbe | undefined>;

/**
 * A guest lxapp (Chat, docked as an aside) speaks only for itself through
 * `lx.host.notification` and `lx.host.banner`: a notification may only open
 * Chat, the Control app cannot dismiss Chat's banner by naming the same id,
 * a banner action may only target Chat, and only the Control app may bring
 * the product forward with `lx.shell.activate`.
 *
 * Picking an action needs the card's accessibility node, which only the macOS
 * card has; on Windows the card is drawn by hand, so that phase runs on macOS.
 */
test('a guest lxapp gets attention for itself only', {
  id: 'DESKTOP-GUEST-ATTENTION-001',
  covers: ['lx.shell.activate'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app, namespace, defer } = bindFixture(t, 'DESKTOP-GUEST-ATTENTION-001');
  const platform = await runtimePlatform(app);
  const desktop = t.automation.desktop;
  const bannerId = `guest-${namespace}`.slice(0, 64);

  defer(async () => {
    await app.logic.eval(async ({ lx }, id) => {
      await lx.host.banner.dismiss(id).catch(() => undefined);
    }, bannerId).catch(() => undefined);
    const manager = t.automation.lxapps;
    const chat = (await manager.list()).find((candidate) => candidate.appId === CHAT_APP_ID);
    if (chat && chat.status !== 'closed') await manager.close({ app: CHAT_APP_ID }).catch(() => undefined);
  });

  await app.logic.eval({ timeout: 20_000 }, async ({ lx }) => {
    await lx.surface.openApp('lingxia-showcase-web', { as: 'aside', edge: 'right', page: 'chat' });
  });
  const chat = t.automation.lxapp(CHAT_APP_ID);

  const offered = await chat.logic.eval(({ lx }) => ({
    banner: typeof lx.host.banner.show,
    supportsBanner: lx.supports('app.banner'),
    notification: typeof lx.host.notification.show,
    activate: typeof lx.shell.activate,
  }));
  expect(offered).toEqual({
    banner: 'function',
    supportsBanner: true,
    notification: 'function',
    activate: 'function',
  });

  // Every refusal lands before anything is posted, so none of these prompts.
  const refused = await chat.logic.eval({ timeout: 20_000 }, async ({ lx }, otherAppId) => {
    const show = lx.host.notification.show as (options: unknown) => Promise<unknown>;
    const code = async (target: unknown) => {
      try {
        await show({ id: 'guest-scope', title: 'scope', target });
        return 'posted';
      } catch (error) {
        return (error as { code?: string }).code ?? String(error);
      }
    };
    const banner = lx.host.banner.show as (options: unknown) => Promise<unknown>;
    const bannerCode = async (target: unknown) => {
      try {
        await banner({ id: 'guest-scope', title: 'scope', actions: [{ id: 'go', label: 'Go', target }] });
        return 'shown';
      } catch (error) {
        return (error as { code?: string }).code ?? String(error);
      }
    };
    let shellActivate = 'returned';
    try {
      lx.shell.activate();
    } catch (error) {
      shellActivate = (error as { code?: string }).code ?? String(error);
    }
    return {
      activate: await code({ kind: 'activate' }),
      page: await code({ kind: 'page', page: 'chat' }),
      route: await code({ kind: 'route', name: 'anything' }),
      appLink: await code({ kind: 'appLink', url: 'https://example.com/x' }),
      otherApp: await code({ kind: 'app', appId: otherAppId }),
      bannerActivate: await bannerCode({ kind: 'activate' }),
      bannerOtherApp: await bannerCode({ kind: 'app', appId: otherAppId }),
      shellActivate,
    };
  }, SHOWCASE_APP_ID);
  expect(refused).toEqual({
    activate: 'E_PERMISSION_DENIED',
    page: 'E_PERMISSION_DENIED',
    route: 'E_PERMISSION_DENIED',
    appLink: 'E_PERMISSION_DENIED',
    otherApp: 'E_PERMISSION_DENIED',
    bannerActivate: 'E_PERMISSION_DENIED',
    bannerOtherApp: 'E_PERMISSION_DENIED',
    shellActivate: 'E_PERMISSION_DENIED',
  });

  // Its own notification, opening itself, under its own id.
  const own = await chat.logic.eval({ timeout: 20_000 }, async ({ lx }) => {
    const n = lx.host.notification;
    const permission = await n.getPermission();
    try {
      const shown = await n.show({
        id: 'guest-own',
        title: 'Run finished',
        body: 'DESKTOP-GUEST-ATTENTION-001',
        target: { app: 'lingxia-showcase-web', params: { run: 'probe' } },
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
    void lx.host.banner.show({
      id,
      title: 'Run finished',
      body: 'DESKTOP-GUEST-ATTENTION-001',
      actions: [{ id: 'open', label: 'Open run', style: 'primary', target: { app: 'lingxia-showcase-web', params: { run: 'probe' } } }],
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
  await app.logic.eval(async ({ lx }, id) => { await lx.host.banner.dismiss(id); }, bannerId);
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
  } else {
    await chat.logic.eval(async ({ lx }, id) => { await lx.host.banner.dismiss(id); }, bannerId);
    expect(await eventually(bannerResult, (value) => value !== null, { describe: 'the guest dismissal' }))
      .toEqual({ status: 'canceled', id: bannerId, reason: 'dismissed' });
  }

  // The Control app brings the product forward without a banner.
  expect(await app.logic.eval(({ lx }) => {
    lx.shell.activate();
    return true;
  })).toBe(true);
});
