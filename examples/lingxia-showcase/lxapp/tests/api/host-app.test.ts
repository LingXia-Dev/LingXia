import { expect, test } from '@lingxia/test';
import { SHOWCASE_APP_ID } from '@lingxia/test/showcase/helpers/app.js';
import { bindFixture, type Caught } from '@lingxia/test/showcase/helpers/poll.js';


// Autostart refuses on mobile hosts. Keep the
// behavioral case for desktop hosts, where the login-item contract exists.
const MOBILE_HOSTS = new Set(['android', 'ios']);
const autostartSkip = MOBILE_HOSTS.has(globalThis.__LINGXIA_AUTOMATION_HOST__?.control?.platform?.toLocaleLowerCase() ?? '')
  ? 'autostart is desktop only'
  : undefined;
const DESKTOP_HOSTS = new Set(['macos', 'windows']);
const bannerSkip = DESKTOP_HOSTS.has(globalThis.__LINGXIA_AUTOMATION_HOST__?.control?.platform?.toLocaleLowerCase() ?? '')
  ? undefined
  : 'host banners are desktop only';

test('publish the lxapp sandbox roots on lx.fs', {
  id: 'ENV-001',
  covers: ['lx.fs.userData', 'lx.fs.userCache'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'ENV-001');

  const env = await app.logic.eval(async ({ lx }) => {
    return {
      data: lx.fs.userData,
      cache: lx.fs.userCache,
      // A second read must return the same roots: they are constants, not probes.
      stable: lx.fs.userData === lx.fs.userData,
    };
  });

  expect(env.data).toMatch(/^lx:\/\//);
  expect(env.cache).toMatch(/^lx:\/\//);
  expect(env.data).not.toBe(env.cache);
  expect(env.stable).toBeTruthy();
});

test('capture a host app screenshot into the lxapp sandbox', {
  id: 'HOSTAPP-SHOT-001',
  covers: ['lx.host', 'lx.host.screenshot'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-SHOT-001');

  const shot = await app.logic.eval(async ({ lx }) => {
    const result = await lx.host.screenshot();
    const stat = await lx.fs.stat(result.path);
    return {
      path: result.path,
      width: result.width ?? null,
      height: result.height ?? null,
      bytes: stat.size,
    };
  });
  t.defer(async () => {
    await app.logic.eval(async ({ lx }, path) => {
      try { await lx.fs.remove(path); } catch {} return true;
    }, shot.path);
  });

  // A screenshot the app cannot read back is not a screenshot.
  expect(shot.path).toMatch(/^lx:\/\//);
  expect(shot.width).toBeGreaterThan(0);
  expect(shot.height).toBeGreaterThan(0);
  expect(shot.bytes).toBeGreaterThan(0);
});

test('set and clear the host app badge without leaving one behind', {
  id: 'HOSTAPP-BADGE-001',
  covers: ['lx.host.setBadge'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-BADGE-001');
  t.defer(async () => {
    await app.logic.eval(async ({ lx }) => {
      try { await lx.host.setBadge(null); } catch {} return true;
    });
  });

  const painted = await app.logic.eval(async ({ lx }) => {
    const first = await lx.host.setBadge(12);
    const cleared = await lx.host.setBadge(null);
    // Clearing an already-clear badge must stay a no-op, not an error.
    const clearedAgain = await lx.host.setBadge(null);
    const empty = await lx.host.setBadge('');
    return [first, cleared, clearedAgain, empty];
  });

  // Support does not guarantee painting: permission and visible chrome vary.
  expect(painted.length).toBe(4);
  for (const result of painted) {
    expect(typeof result.painted).toBe('boolean');
  }
});

test('tell a hidden tray from a shown one in what setBadge reports', {
  id: 'HOSTAPP-BADGE-004',
  covers: ['lx.host.setBadge', 'lx.host.tray.show', 'lx.host.tray.hide'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-BADGE-004');
  t.defer(async () => {
    await app.logic.eval(async ({ lx }) => {
      try { await lx.host.setBadge(null); lx.host.tray.hide(); } catch {} return true;
    });
  });

  // A declared tray has a status item from the start but stays hidden until
  // `show()`, so "the item took the value" is not "the user can see it".
  const hidden = await app.logic.eval(async ({ lx }) => {
    try { lx.host.tray.hide(); } catch {} return (await lx.host.setBadge(4, { surface: 'tray' })).painted;
  });
  const shown = await app.logic.eval(async ({ lx }) => {
    try { lx.host.tray.show(); } catch {} return (await lx.host.setBadge(4, { surface: 'tray' })).painted;
  });

  // A hidden item is never a painted badge, whatever the platform. `shown` is
  // true only where there is a tray at all, so showing may leave it false --
  // but it can never go the other way.
  expect(hidden).toBe(false);
  expect(shown === true || shown === false).toBe(true);
});

test('report a surface this platform does not have instead of failing', {
  id: 'HOSTAPP-BADGE-003',
  covers: ['lx.host.setBadge'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-BADGE-003');
  t.defer(async () => {
    await app.logic.eval(async ({ lx }) => {
      try { await lx.host.setBadge(null); } catch {} return true;
    });
  });

  // A named surface that is absent resolves `no_surface`; it never rejects, so
  // portable code can ask for one without guarding the call.
  const outcome = await app.logic.eval(async ({ lx }): Promise<Caught> => {
    try {
      const value = await lx.host.setBadge(2, { surface: 'tray' });
      return { ok: true, value: value.painted ? 'painted' : value.reason ?? 'declined' };
    } catch (error) {
      const { code, message, data } = error as { code?: string; message?: string; data?: unknown };
      return { ok: false, code: code ?? '', message: String(message ?? error), data: data ?? null };
    }
  });
  expect(outcome.ok).toBeTruthy();
  expect(['painted', 'no_surface', 'declined']).toContain(outcome.value);

  const bad = await app.logic.eval(async ({ lx }): Promise<Caught> => {
    try {
      // `dock` is not a surface the typings offer; the probe asks for it on purpose.
      const value = await (lx.host.setBadge as unknown as (value: number, options: { surface: string }) => Promise<unknown>)(2, { surface: 'dock' });
      return { ok: true, value };
    } catch (error) {
      const { code, message, data } = error as { code?: string; message?: string; data?: unknown };
      return { ok: false, code: code ?? '', message: String(message ?? error), data: data ?? null };
    }
  });
  expect(bad.ok).toBeFalsy();
  expect(bad.code).toBe('E_INVALID_ARG');
});

test('reject a badge value that is neither a string, a number, nor null', {
  id: 'HOSTAPP-BADGE-002',
  covers: ['lx.host.setBadge'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-BADGE-002');
  t.defer(async () => {
    await app.logic.eval(async ({ lx }) => {
      try { await lx.host.setBadge(null); } catch {} return true;
    });
  });

  // Coercing these would paint "[object Object]" on the dock instead of failing.
  for (const literal of ['{ text: \'7\' }', '[1, 2]', 'true', '() => {}']) {
    await t.step(`lx.host.setBadge(${literal})`, async () => {
      const outcome = await app.logic.eval(async ({ lx }, literal): Promise<Caught> => {
        // Values the typings refuse on purpose; a function cannot cross as JSON.
        const values: Record<string, unknown> = {
          "{ text: '7' }": { text: '7' },
          '[1, 2]': [1, 2],
          true: true,
          '() => {}': () => {},
        };
        try {
          await (lx.host.setBadge as unknown as (value: unknown) => Promise<unknown>)(values[literal]);
          return { ok: true };
        } catch (error) {
          const { code, message, data } = error as { code?: string; message?: string; data?: unknown };
          return { ok: false, code: code ?? '', message: String(message ?? error), data: data ?? null };
        }
      }, literal);
      expect(outcome.ok).toBeFalsy();
      expect(outcome.code).toBe('E_INVALID_ARG');
    });
  }
});

test('answer checkUpdate with an update or null instead of throwing', {
  id: 'HOSTAPP-UPDATE-001',
  covers: ['lx.host.checkUpdate', 'lx.app.onUpdateReady', 'lx.app.onUpdateFailed'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-UPDATE-001');

  const result = await app.logic.eval(async ({ lx }) => {
    const decision = await lx.host.checkUpdate();
    const manager = lx.app;
    const ready = manager.onUpdateReady(() => {});
    const failed = manager.onUpdateFailed(() => {});
    ready();
    failed();
    // Unsubscribing twice must stay inert.
    ready();
    return {
      hasUpdate: decision !== null,
      version: decision ? decision.version : null,
      subscriptions: [typeof ready, typeof failed],
    };
  });

  expect(typeof result.hasUpdate).toBe('boolean');
  expect(result.subscriptions).toEqual(['function', 'function']);
  if (result.hasUpdate) expect(typeof result.version).toBe('string');
  else expect(result.version).toBe(null);
});

test('reject an invalid host display language', {
  id: 'HOSTAPP-LANG-002',
  covers: ['lx.host.displayLanguage.setPreference'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-LANG-002');

  const offered = await app.logic.eval(({ lx }) => typeof lx.host.displayLanguage?.setPreference);
  expect(offered).toBe('function');

  for (const language of ['', 'en--US']) {
    const rejected = await app.logic.eval(async ({ lx }, language): Promise<Caught> => {
      try {
        await lx.host.displayLanguage.setPreference(language);
        return { ok: true };
      } catch (error) {
        const { code, message, data } = error as { code?: string; message?: string; data?: unknown };
        return { ok: false, code: code ?? '', message: String(message ?? error), data: data ?? null };
      }
    }, language);
    expect(rejected.ok).toBe(false);
    expect(String(rejected.code)).toBe('E_INVALID_ARG');
  }
});

test('subscribe to and release the display language listener', {
  id: 'HOSTAPP-LANG-001',
  covers: ['lx.host.displayLanguage.watch'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-LANG-001');

  const result = await app.logic.eval(async ({ lx }) => {
    const first = lx.host.displayLanguage.watch(() => {});
    const second = lx.host.displayLanguage.watch(() => {});
    first();
    second();
    first();
    return { kinds: [typeof first, typeof second], distinct: first !== second };
  });

  expect(result.kinds).toEqual(['function', 'function']);
  expect(result.distinct).toBeTruthy();
});

test('request permission and replace local notifications by id', {
  id: 'HOSTAPP-NOTIFICATION-001',
  covers: [
    'lx.host.notification',
    'lx.host.notification.getPermission',
    'lx.host.notification.requestPermission',
    'lx.host.notification.show',
    'lx.host.notification.cancel',
    'lx.host.notification.cancelAll',
  ],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-NOTIFICATION-001');

  // The showcase host declares notifications, so every platform must report them.
  expect(await app.logic.eval(({ lx }) => lx.supports('app.notification'))).toBe(true);

  const result = await app.logic.eval({ timeout: 30_000 }, async ({ lx }) => {
    const n = lx.host.notification;
    const permission = await n.getPermission();
    // A pending OS prompt has nobody to answer it here, so only ask when
    // the answer is already known.
    const requested = permission === 'default' ? null : await n.requestPermission();
    let immediate = null;
    try {
      immediate = await n.show({
        id: 'automation-local',
        title: 'LingXia automation',
        message: 'suppressed while frontmost',
        silent: true,
      });
    } catch (error) {
      if (permission === 'granted') throw error;
    }
    let scheduled = null;
    let replaced = null;
    let scheduleUnsupported = false;
    // A platform that cannot schedule for this app says so with E_NOT_SUPPORTED
    // (HarmonyOS without the agent-reminder privilege), never a generic error.
    const orUnsupported = async <T,>(call: () => Promise<T>): Promise<T | null> => {
      try {
        return await call();
      } catch (error) {
        if ((error as { code?: string }).code !== 'E_NOT_SUPPORTED') throw error;
        scheduleUnsupported = true;
        return null;
      }
    };
    if (permission === 'granted') {
      scheduled = await orUnsupported(() => n.show({
        id: 'automation-local',
        title: 'LingXia automation',
        message: 'scheduled',
        schedule: { delayMs: 60_000 },
        silent: true,
      }));
      replaced = await orUnsupported(() => n.show({
        id: 'automation-local',
        title: 'LingXia automation',
        message: 'replaces the schedule',
        schedule: { at: Date.now() + 120_000 },
        silent: true,
      }));
    }
    await n.cancel('automation-local');
    await n.cancel('never-shown');
    await n.cancelAll();
    // Options the typings refuse on purpose.
    const show = (options: unknown) => (n.show as (options: unknown) => Promise<unknown>)(options);
    const rejects = async (options: unknown) => {
      try { await show(options); return false; } catch { return true; }
    };
    return {
      permission,
      requested,
      immediate,
      scheduled,
      replaced,
      scheduleUnsupported,
      rejected: {
        http: await rejects({ title: 'bad', applink: 'http://example.com/x' }),
        page: await rejects({ title: 'bad', target: { kind: 'page', page: '/pages/system/index' } }),
        title: await rejects({ message: 'no title' }),
        both: await rejects({ title: 'bad', schedule: { at: Date.now() + 1000, delayMs: 5 } }),
        emptyId: await rejects({ id: '', title: 'bad' }),
      },
    };
  });

  expect(['granted', 'denied', 'default']).toContain(result.permission);
  if (result.requested !== null) {
    expect(result.requested).toBe(result.permission);
  }
  if (result.immediate) {
    expect(result.immediate.id).toBe('automation-local');
    expect(['posted', 'suppressed']).toContain(result.immediate.delivery);
  }
  if (result.permission === 'granted' && !result.scheduleUnsupported) {
    expect(result.scheduled).toEqual({ id: 'automation-local', delivery: 'scheduled' });
    expect(result.replaced).toEqual({ id: 'automation-local', delivery: 'scheduled' });
  }
  expect(result.rejected).toEqual({ http: true, page: true, title: true, both: true, emptyId: true });
});

test('show a toast, dismiss a prompt, and reject bad banner options', {
  skip: bannerSkip,
  id: 'HOSTAPP-BANNER-001',
  covers: [
    'lx.host.banner',
    'lx.host.banner.show',
    'lx.host.banner.dismiss',
  ],
  app: SHOWCASE_APP_ID,
  reason: 'Desktop banner is Control-app / macOS / Windows only.',
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-BANNER-001');
  t.defer(async () => {
    await app.logic.eval(async ({ lx }) => {
      try {
        await lx.host.banner.dismiss('automation-banner-toast');
        await lx.host.banner.dismiss('automation-banner-prompt');
      } catch {}
            return true;
    });
  });

  const offered = await app.logic.eval(({ lx }) => !!(lx.host.banner && typeof lx.host.banner.show === 'function'));
  expect(offered).toBe(true);
  const supported = await app.logic.eval(({ lx }) => !!lx.supports('app.banner'));
  expect(supported).toBe(true);

  const result = await app.logic.eval({ timeout: 20_000 }, async ({ lx }) => {
    const banner = lx.host.banner;
    const toast = await banner.show({
      id: 'automation-banner-toast',
      title: 'LingXia automation',
      message: 'desktop card',
      timeoutMs: 800,
    });
    const prompt = banner.show({
      id: 'automation-banner-prompt',
      title: 'Allow this action?',
      message: 'agent consent',
      actions: [
        { id: 'deny', label: 'Deny' },
        { id: 'allow', label: 'Allow', tone: 'primary' },
      ],
      timeoutMs: 0,
    });
    await banner.dismiss('automation-banner-prompt');
    await banner.dismiss('never-shown');
    const promptResult = await prompt;
    // Options the typings refuse on purpose.
    const show = (options: unknown) => (banner.show as (options: unknown) => Promise<unknown>)(options);
    const rejects = async (options: unknown) => {
      try { await show(options); return false; } catch { return true; }
    };
    return {
      toast,
      prompt: promptResult,
      rejected: {
        title: await rejects({ message: 'no title' }),
        emptyId: await rejects({ id: '', title: 'bad' }),
        tooMany: await rejects({
          title: 'bad',
          actions: [
            { id: 'a', label: 'A' },
            { id: 'b', label: 'B' },
            { id: 'c', label: 'C' },
          ],
        }),
        background: await rejects({ title: 'bad', background: 'blurple' }),
      },
    };
  });

  expect(result.toast).toEqual({
    id: 'automation-banner-toast',
    status: 'canceled',
    reason: 'timeout',
  });
  expect(result.prompt).toEqual({
    id: 'automation-banner-prompt',
    status: 'canceled',
    reason: 'dismissed',
  });
  expect(result.rejected).toEqual({ title: true, emptyId: true, tooMany: true, background: true });
});

test('report autostart state and accept an idempotent write', {
  skip: autostartSkip,
  id: 'HOSTAPP-AUTOSTART-001',
  covers: ['lx.host.autostart', 'lx.host.autostart.isEnabled', 'lx.host.autostart.enable', 'lx.host.autostart.disable'],
  app: SHOWCASE_APP_ID,
  reason: 'Autostart is intentionally absent on mobile hosts.',
  // `SMAppService.mainApp.status` costs ~6s per call on macOS; the spec pays
  // that twice rather than pretending the API is cheap.
  timeout: 90_000,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-AUTOSTART-001');

  // The member is always present; a host without login items refuses the call.
  const probe = await app.logic.eval(async ({ lx }) => ({
    desktop: lx.host.info.os === 'macOS' || lx.host.info.os === 'Windows',
    supported: lx.supports('app.autostart'),
    refusal: lx.supports('app.autostart') ? null
      : await lx.host.autostart.isEnabled().then(() => 'resolved', (error: { code?: string }) => error?.code ?? String(error)),
  }));
  expect(probe.supported).toBe(probe.desktop);
  if (!probe.supported) {
    expect(probe.refusal).toBe('E_NOT_SUPPORTED');
    return;
  }

  // Writing the value the host already holds proves the setter without
  // registering or removing a real login item on the developer's machine.
  // The macOS login-item service answers well past an eval's default budget.
  const result = await app.logic.eval({ timeout: 45_000 }, async ({ lx }) => {
    const before = await lx.host.autostart.isEnabled();
    await (before ? lx.host.autostart.enable() : lx.host.autostart.disable());
    const after = await lx.host.autostart.isEnabled();
    return { before, after };
  });

  expect(typeof result.before).toBe('boolean');
  expect(result.after).toBe(result.before);
});

test('clear product caches while preserving live app storage', {
  id: 'HOSTAPP-CACHE-001',
  covers: ['lx.host.cache', 'lx.host.cache.size', 'lx.host.cache.clear'],
  app: SHOWCASE_APP_ID,
  // A clear walks every lxapp's storage and asks the WebView store to drop its
  // cache; both are slower than a plain property read.
  timeout: 90_000,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-CACHE-001');

  const result = await app.logic.eval({ timeout: 60_000 }, async ({ lx }) => {
    const payload = 'x'.repeat(256 * 1024);
    const cached = lx.fs.userCache + '/automation/cache-probe.txt';
    const durable = 'automation/cache-probe.txt';
    await lx.fs.write(cached, { data: payload, overwrite: true });
    await lx.fs.write(durable, { data: payload, overwrite: true });

    await lx.storage().set('automation-cache-probe', 'keep');
    const before = await lx.host.cache.size();
    const report = await lx.host.cache.clear();
    const after = await lx.host.cache.size();

    const cachedSurvived = await lx.fs.exists(cached);
    const durableSurvived = await lx.fs.exists(durable);
    const kv = await lx.storage().get('automation-cache-probe');
    await lx.storage().delete('automation-cache-probe');
    await lx.fs.remove(durable);
    await lx.fs.remove(cached);

    return { before, report, after, cachedSurvived, durableSurvived, kv };
  });

  expect(result.before).toBeGreaterThanOrEqual(0);
  expect(result.report.freedBytes).toBeGreaterThanOrEqual(0);
  expect(result.report.skippedActivePaths).toBeGreaterThanOrEqual(1);
  expect(['cleared', 'unsupported', 'failed']).toContain(result.report.webview);
  expect(result.after).toBeGreaterThanOrEqual(0);
  expect(result.cachedSurvived).toBe(true);
  expect(result.durableSurvived).toBe(true);
  expect(result.kv).toBe('keep');
});

test('show, label, and retract the host tray item', {
  id: 'HOSTAPP-TRAY-001',
  covers: [
    'lx.host.tray',
    'lx.host.tray.show',
    'lx.host.tray.hide',
    'lx.host.tray.setTitle',
    'lx.host.tray.setMenu',
    'lx.host.tray.setIcon',
    'lx.host.tray.onClick',
  ],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-TRAY-001');
  t.defer(async () => {
    await app.logic.eval(async ({ lx }) => {
      try { lx.host.tray.hide(); } catch {} return true;
    });
  });

  const result = await app.logic.eval(async ({ lx }) => {
    lx.host.tray.show();
    lx.host.tray.setTitle('LX');
    lx.host.tray.setIcon('public/showcase-icon.svg');
    lx.host.tray.setMenu([
      { label: 'Open Showcase', onClick: () => {} },
      { separator: true },
      { label: 'Disabled', enabled: false },
    ]);
    const off = lx.host.tray.onClick(() => {});
    off();
    lx.host.tray.setTitle(null);
    lx.host.tray.hide();
    // Hiding a hidden tray stays a no-op.
    lx.host.tray.hide();
    return { unsubscribe: typeof off };
  });

  expect(result.unsubscribe).toBe('function');
});

test('declare, patch, and retract runtime sidebar actions atomically', {
  id: 'HOSTAPP-SIDEBAR-001',
  covers: [
    'lx.shell',
    'lx.shell.sidebarActions',
    'lx.shell.sidebarActions.replace',
    'lx.shell.sidebarActions.update',
    'lx.shell.sidebarActions.remove',
  ],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-SIDEBAR-001');
  // The declaration is process-local shell chrome; drop it however the spec ends.
  t.defer(async () => {
    await app.logic.eval(async ({ lx }) => {
      const probe = globalThis as { __sidebarProbeIcon?: string };
      try { await lx.shell.sidebarActions.replace([]); } catch {}
      if (probe.__sidebarProbeIcon) {
        try { await lx.fs.remove(probe.__sidebarProbeIcon); } catch {}
        delete probe.__sidebarProbeIcon;
      }
      return true;
    });
  });

  await t.step('declare a header and a footer action', async () => {
    const result = await app.logic.eval(async ({ lx }): Promise<Caught> => {
      try {
        await lx.shell.sidebarActions.replace([
          { id: 'probe-header', placement: 'header', icon: 'public/showcase-icon.svg', label: 'Probe header', onActivate() {} },
          { id: 'probe-footer', placement: 'footer', icon: 'public/showcase-icon.svg', label: 'Probe footer', onActivate() {} },
        ]);
        return { ok: true };
      } catch (error) {
        const { code, message, data } = error as { code?: string; message?: string; data?: unknown };
        return { ok: false, code: code ?? '', message: String(message ?? error), data: data ?? null };
      }
    });
    expect(result.ok).toBeTruthy();
  });

  await t.step('patch presentation of one live id', async () => {
    const result = await app.logic.eval(async ({ lx }): Promise<Caught> => {
      try {
        await lx.shell.sidebarActions.update('probe-footer', { label: 'Probe footer 2', disabled: true });
        return { ok: true };
      } catch (error) {
        const { code, message, data } = error as { code?: string; message?: string; data?: unknown };
        return { ok: false, code: code ?? '', message: String(message ?? error), data: data ?? null };
      }
    });
    expect(result.ok).toBeTruthy();
  });

  await t.step('accept a runtime-managed raster icon', async () => {
    const result = await app.logic.eval(async ({ lx }): Promise<Caught> => {
      try {
        const shot = await lx.host.screenshot();
        (globalThis as { __sidebarProbeIcon?: string }).__sidebarProbeIcon = shot.path;
        await lx.shell.sidebarActions.update('probe-footer', { icon: shot.path });
        const value = shot.path;
        return { ok: true, value };
      } catch (error) {
        const { code, message, data } = error as { code?: string; message?: string; data?: unknown };
        return { ok: false, code: code ?? '', message: String(message ?? error), data: data ?? null };
      }
    });
    expect(result.ok).toBeTruthy();
    expect(result.value).toMatch(/^lx:\/\//);
  });

  await t.step('keep settings-shaped declarations on the generic runtime channel', async () => {
    const result = await app.logic.eval(async ({ lx }): Promise<Caught> => {
      try {
        await lx.shell.sidebarActions.replace([
          { id: 'settings', placement: 'footer', icon: 'public/showcase-icon.svg', label: 'ID spoof', onActivate() {} },
          { id: 'label-spoof', placement: 'footer', icon: 'public/showcase-icon.svg', label: 'Settings', onActivate() {} },
          { id: 'icon-spoof', placement: 'footer', icon: 'public/sidebar-settings.svg', label: 'Icon spoof', onActivate() {} },
        ]);
        await lx.shell.sidebarActions.update('settings', { label: 'ID spoof live' });
        await lx.shell.sidebarActions.update('label-spoof', { disabled: true });
        await lx.shell.sidebarActions.update('icon-spoof', { label: 'Icon spoof live' });
        return { ok: true };
      } catch (error) {
        const { code, message, data } = error as { code?: string; message?: string; data?: unknown };
        return { ok: false, code: code ?? '', message: String(message ?? error), data: data ?? null };
      }
    });
    expect(result.ok).toBeTruthy();

    // Restore the live set used by the following rollback assertions.
    const restored = await app.logic.eval(async ({ lx }): Promise<Caught> => {
      try {
        await lx.shell.sidebarActions.replace([
          { id: 'probe-header', placement: 'header', icon: 'public/showcase-icon.svg', label: 'Probe header', onActivate() {} },
          { id: 'probe-footer', placement: 'footer', icon: 'public/showcase-icon.svg', label: 'Probe footer 2', disabled: true, onActivate() {} },
        ]);
        return { ok: true };
      } catch (error) {
        const { code, message, data } = error as { code?: string; message?: string; data?: unknown };
        return { ok: false, code: code ?? '', message: String(message ?? error), data: data ?? null };
      }
    });
    expect(restored.ok).toBeTruthy();
  });

  await t.step('reject a patch for an id outside the declaration', async () => {
    const result = await app.logic.eval(async ({ lx }): Promise<Caught> => {
      try {
        await lx.shell.sidebarActions.update('probe-missing', { label: 'nope' });
        return { ok: true };
      } catch (error) {
        const { code, message, data } = error as { code?: string; message?: string; data?: unknown };
        return { ok: false, code: code ?? '', message: String(message ?? error), data: data ?? null };
      }
    });
    expect(result.ok).toBeFalsy();
    expect(result.code).toBe('E_NOT_FOUND');
  });

  await t.step('reject a third header action without disturbing the live set', async () => {
    const result = await app.logic.eval(async ({ lx }): Promise<Caught> => {
      // A refusal settles the returned promise; it never throws at the call.
      let pending: Promise<void>;
      try {
        pending = lx.shell.sidebarActions.replace([
          { id: 'h1', placement: 'header', icon: 'public/showcase-icon.svg', label: 'One', onActivate() {} },
          { id: 'h2', placement: 'header', icon: 'public/showcase-icon.svg', label: 'Two', onActivate() {} },
          { id: 'h3', placement: 'header', icon: 'public/showcase-icon.svg', label: 'Three', onActivate() {} },
        ]);
      } catch (error) {
        return { ok: false, code: 'THREW', message: String(error), data: null };
      }
      try {
        await pending;
        return { ok: true };
      } catch (error) {
        const { code, message, data } = error as { code?: string; message?: string; data?: unknown };
        return { ok: false, code: code ?? '', message: String(message ?? error), data: data ?? null };
      }
    });
    expect(result.ok).toBeFalsy();
    expect(result.code).toBe('E_INVALID_ARG');
    // The rejected generation must not have replaced the live one.
    const survivor = await app.logic.eval(async ({ lx }): Promise<Caught> => {
      try {
        await lx.shell.sidebarActions.update('probe-header', { label: 'Probe header 2' });
        return { ok: true };
      } catch (error) {
        const { code, message, data } = error as { code?: string; message?: string; data?: unknown };
        return { ok: false, code: code ?? '', message: String(message ?? error), data: data ?? null };
      }
    });
    expect(survivor.ok).toBeTruthy();
  });

  await t.step('set, clear, and reject footer values', async () => {
    const result = await app.logic.eval(async ({ lx }) => {
      const attempt = async (run: () => Promise<void>) => {
        try {
          await run();
          return { ok: true, code: '', message: '' };
        } catch (error) {
          const { code, message } = error as { code?: string; message?: string };
          return { ok: false, code: code ?? '', message: String(message ?? error) };
        }
      };
      const missing: { name: string } | undefined = undefined as { name: string } | undefined;
      return {
        set: await attempt(() => lx.shell.sidebarActions.update('probe-footer', { value: 'Acme' })),
        // A present key clears even when its value is undefined.
        clearUndefined: await attempt(() => lx.shell.sidebarActions.update('probe-footer', { value: missing?.name })),
        clearNull: await attempt(() => lx.shell.sidebarActions.update('probe-footer', { value: null })),
        header: await attempt(() => lx.shell.sidebarActions.update('probe-header', { value: 'Acme' })),
        third: await attempt(() => lx.shell.sidebarActions.replace([
          { id: 'v1', placement: 'footer', icon: 'public/showcase-icon.svg', label: 'One', value: 'a', onActivate() {} },
          { id: 'v2', placement: 'footer', icon: 'public/showcase-icon.png', label: 'Two', value: 'b', onActivate() {} },
          { id: 'v3', placement: 'footer', icon: 'public/sidebar-chat.svg', label: 'Three', value: 'c', onActivate() {} },
        ])),
      };
    });
    expect(result.set.ok).toBeTruthy();
    expect(result.clearUndefined.ok).toBeTruthy();
    expect(result.clearNull.ok).toBeTruthy();
    expect(result.header.ok).toBeFalsy();
    expect(result.header.code).toBe('E_INVALID_ARG');
    expect(result.header.message).toContain('footer-only');
    expect(result.third.ok).toBeFalsy();
    expect(result.third.code).toBe('E_INVALID_ARG');
    expect(result.third.message).toContain('v1, v2, v3');
  });

  await t.step('remove one id, then clear the rest idempotently with replace([])', async () => {
    const removed = await app.logic.eval(async ({ lx }): Promise<Caught> => {
      try {
        await lx.shell.sidebarActions.remove('probe-footer');
        return { ok: true };
      } catch (error) {
        const { code, message, data } = error as { code?: string; message?: string; data?: unknown };
        return { ok: false, code: code ?? '', message: String(message ?? error), data: data ?? null };
      }
    });
    expect(removed.ok).toBeTruthy();

    const removedTwice = await app.logic.eval(async ({ lx }): Promise<Caught> => {
      try {
        await lx.shell.sidebarActions.remove('probe-footer');
        return { ok: true };
      } catch (error) {
        const { code, message, data } = error as { code?: string; message?: string; data?: unknown };
        return { ok: false, code: code ?? '', message: String(message ?? error), data: data ?? null };
      }
    });
    expect(removedTwice.ok).toBeFalsy();
    expect(removedTwice.code).toBe('E_NOT_FOUND');

    const cleared = await app.logic.eval(async ({ lx }): Promise<Caught> => {
      try {
        await lx.shell.sidebarActions.replace([]);
        await lx.shell.sidebarActions.replace([]);
        return { ok: true };
      } catch (error) {
        const { code, message, data } = error as { code?: string; message?: string; data?: unknown };
        return { ok: false, code: code ?? '', message: String(message ?? error), data: data ?? null };
      }
    });
    expect(cleared.ok).toBeTruthy();
  });
});

test('set and restore the name and icon the host shows for this app', {
  id: 'HOSTAPP-APP-DISPLAY-001',
  covers: ['lx.shell.setAppDisplay'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-APP-DISPLAY-001');
  // The choice persists across launches; restore the defaults however the spec ends.
  t.defer(async () => {
    await app.logic.eval(async ({ lx }) => {
      try { await lx.shell.setAppDisplay({ name: null, icon: null }); } catch {}
      return true;
    });
  });

  const result = await app.logic.eval(async ({ lx }) => {
    const setDisplay = lx.shell.setAppDisplay as (patch: unknown) => Promise<void>;
    const attempt = async (patch: unknown) => {
      try {
        await setDisplay(patch);
        return { ok: true, code: '' };
      } catch (error) {
        return { ok: false, code: (error as { code?: string }).code ?? '' };
      }
    };
    const packageName = lx.app.info.name;
    const set = await attempt({ name: '  Showcase Brand  ', icon: 'public/showcase-icon.png' });
    const again = await attempt({ name: 'Showcase Brand', icon: 'public/showcase-icon.png' });
    return {
      set,
      again,
      svg: await attempt({ icon: 'public/showcase-icon.svg' }),
      nameAfterSet: lx.app.info.name === packageName,
      empty: await attempt({}),
      unknown: await attempt({ title: 'x' }),
      notObject: await attempt('x'),
      multiline: await attempt({ name: 'a\nb' }),
      tooLong: await attempt({ name: 'x'.repeat(65) }),
      remote: await attempt({ icon: 'https://example.com/a.png' }),
      traversal: await attempt({ icon: '../a.png' }),
      notImage: await attempt({ icon: 'lxapp.json' }),
      blankClears: await attempt({ name: '   ', icon: '' }),
    };
  });
  expect(result.set.ok).toBeTruthy();
  expect(result.again.ok).toBeTruthy();
  expect(result.svg.ok).toBeTruthy();
  expect(result.nameAfterSet).toBeTruthy();
  for (const refused of [
    result.empty, result.unknown, result.notObject, result.multiline, result.tooLong,
    result.remote, result.traversal, result.notImage,
  ]) {
    expect(refused).toEqual({ ok: false, code: 'E_INVALID_ARG' });
  }
  expect(result.blankClears.ok).toBeTruthy();
});

test('refuse to move the root main out of main', {
  id: 'HOSTAPP-SHELL-OPENAPP-ROOT-001',
  covers: ['lx.surface.openApp'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-SHELL-OPENAPP-ROOT-001');

  const result = await app.logic.eval(async ({ lx }): Promise<Caught> => {
    try {
      await lx.surface.openApp(lx.app.info.appId, { as: 'aside' });
      return { ok: true };
    } catch (error) {
      const { code, message, data } = error as { code?: string; message?: string; data?: unknown };
      return { ok: false, code: code ?? '', message: String(message ?? error), data: data ?? null };
    }
  });

  expect(result.ok).toBeFalsy();
  expect(result.code).toBe('E_SURFACE_ROLE_LOCKED');
});

test('read the page size class and release its watcher', {
  id: 'HOSTAPP-SURFACE-CTX-001',
  covers: ['lx.surface'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-SURFACE-CTX-001');

  const result = await app.logic.eval(({ currentPage }) => {
    const env = currentPage()!.env;
    const first = env.watch(() => {});
    const second = env.watch(() => {});
    first();
    second();
    // Releasing twice must stay inert rather than throwing.
    first();
    return {
      kinds: [typeof first, typeof second], distinct: first !== second,
      sizeClass: env.sizeClass, canDock: typeof env.canDock,
    };
  });

  expect(result.kinds).toEqual(['function', 'function']);
  expect(result.distinct).toBeTruthy();
  expect(['compact', 'regular']).toContain(result.sizeClass);
  expect(result.canDock).toBe('boolean');
});
