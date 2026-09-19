import { expect, spec } from '@lingxia/test';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { bindFixture, evalCaught } from '../helpers/poll.js';

const testArgs = globalThis.__LINGXIA_AUTOMATION_HOST__?.args
  ?? {} as Record<string, string>;
// The published API intentionally omits autostart on mobile hosts. Keep the
// behavioral case for desktop hosts, where the login-item contract exists.
const MOBILE_HOSTS = new Set(['android', 'ios']);
const autostartSpec = MOBILE_HOSTS.has(testArgs.platform?.toLocaleLowerCase() ?? '')
  ? spec.skip
  : spec;
const DESKTOP_HOSTS = new Set(['macos', 'windows']);
const bannerSpec = DESKTOP_HOSTS.has(testArgs.platform?.toLocaleLowerCase() ?? '')
  ? spec
  : spec.skip;

spec('publish the lxapp sandbox roots through lx.env', {
  id: 'ENV-001',
  covers: ['lx.env', 'lx.env.USER_DATA_PATH', 'lx.env.USER_CACHE_PATH'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'ENV-001');

  const env = await app.eval({
    script: `return {
      keys: Object.keys(lx.env).sort(),
      data: lx.env.USER_DATA_PATH,
      cache: lx.env.USER_CACHE_PATH,
      // A second read must return the same roots — env is a snapshot, not a probe.
      stable: lx.env.USER_DATA_PATH === lx.env.USER_DATA_PATH,
    };`,
  }) as { keys: string[]; data: string; cache: string; stable: boolean };

  expect(env.keys).toEqual(['USER_CACHE_PATH', 'USER_DATA_PATH']);
  expect(env.data).toMatch(/^lx:\/\//);
  expect(env.cache).toMatch(/^lx:\/\//);
  expect(env.data).not.toBe(env.cache);
  expect(env.stable).toBeTruthy();
});

spec('capture a host app screenshot into the lxapp sandbox', {
  id: 'HOSTAPP-SHOT-001',
  covers: ['lx.app', 'lx.app.screenshot'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-SHOT-001');

  const shot = await app.eval({
    script: `
      const result = await lx.app.screenshot();
      const stat = await lx.fs.stat(result.tempFilePath);
      return {
        path: result.tempFilePath,
        width: result.width,
        height: result.height,
        bytes: stat.size,
      };
    `,
  }) as { path: string; width: number; height: number; bytes: number };
  t.defer(async () => {
    await app.eval({
      script: `try { await lx.fs.remove(${JSON.stringify(shot.path)}); } catch {} return true;`,
    });
  });

  // A screenshot the app cannot read back is not a screenshot.
  expect(shot.path).toMatch(/^lx:\/\//);
  expect(shot.width).toBeGreaterThan(0);
  expect(shot.height).toBeGreaterThan(0);
  expect(shot.bytes).toBeGreaterThan(0);
});

spec('set and clear the host app badge without leaving one behind', {
  id: 'HOSTAPP-BADGE-001',
  covers: ['lx.app.setBadge'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-BADGE-001');
  t.defer(async () => {
    await app.eval({ script: `try { lx.app.setBadge(null); } catch {} return true;` });
  });

  const result = await app.eval({
    script: `
      lx.app.setBadge('7');
      lx.app.setBadge(12);
      lx.app.setBadge(null);
      // Clearing an already-clear badge must stay a no-op, not an error.
      lx.app.setBadge(null);
      lx.app.setBadge('');
      return 'ok';
    `,
  });

  expect(result).toBe('ok');
});

spec('reject a badge value that is neither a string, a number, nor null', {
  id: 'HOSTAPP-BADGE-002',
  covers: ['lx.app.setBadge', 'lx.tray.setBadge'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-BADGE-002');
  t.defer(async () => {
    await app.eval({ script: `try { lx.app.setBadge(null); lx.tray.setBadge(null); } catch {} return true;` });
  });

  // Coercing these would paint "[object Object]" on the dock instead of failing.
  for (const literal of ['{ text: \'7\' }', '[1, 2]', 'true', '() => {}']) {
    await t.step(`lx.app.setBadge(${literal})`, async () => {
      const outcome = await evalCaught(app, `lx.app.setBadge(${literal}); return 'accepted';`);
      expect(outcome.ok).toBeFalsy();
      expect(outcome.code).toBe('E_INVALID_ARG');
    });
    await t.step(`lx.tray.setBadge(${literal})`, async () => {
      const outcome = await evalCaught(app, `lx.tray.setBadge(${literal}); return 'accepted';`);
      expect(outcome.ok).toBeFalsy();
      expect(outcome.code).toBe('E_INVALID_ARG');
    });
  }
});

spec('answer checkUpdate with a decision instead of throwing', {
  id: 'HOSTAPP-UPDATE-001',
  covers: ['lx.app.checkUpdate', 'lx.getUpdateManager', 'UpdateManager.onUpdateReady', 'UpdateManager.onUpdateFailed'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-UPDATE-001');

  const result = await app.eval({
    script: `
      const decision = await lx.app.checkUpdate();
      const manager = lx.getUpdateManager();
      const ready = manager.onUpdateReady(() => {});
      const failed = manager.onUpdateFailed(() => {});
      ready();
      failed();
      // Unsubscribing twice must stay inert.
      ready();
      return {
        hasUpdate: decision.hasUpdate,
        version: decision.version ?? null,
        subscriptions: [typeof ready, typeof failed],
      };
    `,
  }) as { hasUpdate: boolean; version: string | null; subscriptions: string[] };

  expect(typeof result.hasUpdate).toBe('boolean');
  expect(result.subscriptions).toEqual(['function', 'function']);
  if (result.hasUpdate) expect(typeof result.version).toBe('string');
  else expect(result.version).toBe(null);
});

spec('reject an invalid host display language', {
  id: 'HOSTAPP-LANG-002',
  covers: ['lx.app.control.displayLanguage.setPreference'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-LANG-002');

  const offered = await app.eval({
    script: `return typeof lx.app.control?.displayLanguage?.setPreference`,
  }) as string;
  expect(offered).toBe('function');

  for (const language of ['', 'en--US']) {
    const rejected = await evalCaught(
      app,
      `await lx.app.control.displayLanguage.setPreference(${JSON.stringify(language)})`,
    );
    expect(rejected.ok).toBe(false);
    expect(String(rejected.code)).toBe('E_INVALID_ARG');
  }
});

spec('subscribe to and release the display language listener', {
  id: 'HOSTAPP-LANG-001',
  covers: ['lx.app.displayLanguage.watch'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-LANG-001');

  const result = await app.eval({
    script: `
      const first = lx.app.displayLanguage.watch(() => {});
      const second = lx.app.displayLanguage.watch(() => {});
      first();
      second();
      first();
      return { kinds: [typeof first, typeof second], distinct: first !== second };
    `,
  }) as { kinds: string[]; distinct: boolean };

  expect(result.kinds).toEqual(['function', 'function']);
  expect(result.distinct).toBeTruthy();
});

spec('request permission and replace local notifications by id', {
  id: 'HOSTAPP-NOTIFICATION-001',
  covers: [
    'lx.app.notification',
    'lx.app.notification.getPermission',
    'lx.app.notification.requestPermission',
    'lx.app.notification.show',
    'lx.app.notification.cancel',
    'lx.app.notification.cancelAll',
  ],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-NOTIFICATION-001');

  const offered = await app.eval({
    script: `return !!(lx.app.notification && typeof lx.app.notification.show === 'function')`,
  }) as boolean;
  expect(offered).toBe(true);
  const supported = await app.eval({ script: `return !!lx.supports({ capability: 'notifications' })` });
  expect(supported).toBe(true);

  const result = await app.eval({
    timeoutMs: 30_000,
    script: `
      const n = lx.app.notification;
      const permission = await n.getPermission();
      // A pending OS prompt has nobody to answer it here, so only ask when
      // the answer is already known.
      const requested = permission === 'default' ? null : await n.requestPermission();
      let immediate = null;
      try {
        immediate = await n.show({
          id: 'automation-local',
          title: 'LingXia automation',
          body: 'suppressed while frontmost',
          silent: true,
        });
      } catch (error) {
        if (permission === 'granted') throw error;
      }
      let scheduled = null;
      let replaced = null;
      if (permission === 'granted') {
        scheduled = await n.show({
          id: 'automation-local',
          title: 'LingXia automation',
          body: 'scheduled',
          schedule: { delayMs: 60_000 },
          silent: true,
        });
        replaced = await n.show({
          id: 'automation-local',
          title: 'LingXia automation',
          body: 'replaces the schedule',
          schedule: { at: Date.now() + 120_000 },
          silent: true,
        });
      }
      await n.cancel('automation-local');
      await n.cancel('never-shown');
      await n.cancelAll();
      const rejects = async (options) => {
        try { await n.show(options); return false; } catch { return true; }
      };
      return {
        permission,
        requested,
        immediate,
        scheduled,
        replaced,
        rejected: {
          http: await rejects({ title: 'bad', applink: 'http://example.com/x' }),
          title: await rejects({ body: 'no title' }),
          both: await rejects({ title: 'bad', schedule: { at: Date.now() + 1000, delayMs: 5 } }),
          emptyId: await rejects({ id: '', title: 'bad' }),
        },
      };
    `,
  }) as {
    permission: 'granted' | 'denied' | 'default';
    requested: 'granted' | 'denied' | null;
    immediate: { id: string; status: string } | null;
    scheduled: { id: string; status: string } | null;
    replaced: { id: string; status: string } | null;
    rejected: Record<string, boolean>;
  };

  expect(['granted', 'denied', 'default']).toContain(result.permission);
  if (result.requested !== null) {
    expect(result.requested).toBe(result.permission);
  }
  if (result.immediate) {
    expect(result.immediate.id).toBe('automation-local');
    expect(['shown', 'suppressed']).toContain(result.immediate.status);
  }
  if (result.permission === 'granted') {
    expect(result.scheduled).toEqual({ id: 'automation-local', status: 'scheduled' });
    expect(result.replaced).toEqual({ id: 'automation-local', status: 'scheduled' });
  }
  expect(result.rejected).toEqual({ http: true, title: true, both: true, emptyId: true });
});

bannerSpec('show a toast, dismiss a prompt, and reject bad banner options', {
  id: 'HOSTAPP-BANNER-001',
  covers: [
    'lx.app.banner',
    'lx.app.banner.show',
    'lx.app.banner.dismiss',
  ],
  app: SHOWCASE_APP_ID,
  reason: 'Desktop banner is Control-app / macOS / Windows only.',
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-BANNER-001');
  t.defer(async () => {
    await app.eval({
      script: `try {
        await lx.app.banner.dismiss('automation-banner-toast');
        await lx.app.banner.dismiss('automation-banner-prompt');
      } catch {}
      return true;`,
    });
  });

  const offered = await app.eval({
    script: `return !!(lx.app.banner && typeof lx.app.banner.show === 'function')`,
  }) as boolean;
  expect(offered).toBe(true);
  const supported = await app.eval({ script: `return !!lx.supports({ capability: 'banner' })` });
  expect(supported).toBe(true);

  const result = await app.eval({
    timeoutMs: 20_000,
    script: `
      const banner = lx.app.banner;
      const toast = await banner.show({
        id: 'automation-banner-toast',
        title: 'LingXia automation',
        body: 'desktop card',
        timeoutMs: 800,
      });
      const prompt = banner.show({
        id: 'automation-banner-prompt',
        title: 'Allow this action?',
        body: 'agent consent',
        actions: [
          { id: 'deny', label: 'Deny' },
          { id: 'allow', label: 'Allow', style: 'primary' },
        ],
        timeoutMs: 0,
      });
      await banner.dismiss('automation-banner-prompt');
      await banner.dismiss('never-shown');
      const promptResult = await prompt;
      const rejects = async (options) => {
        try { await banner.show(options); return false; } catch { return true; }
      };
      return {
        toast,
        prompt: promptResult,
        rejected: {
          title: await rejects({ body: 'no title' }),
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
    `,
  }) as {
    toast: { id: string; canceled: boolean; reason?: string; action?: string };
    prompt: { id: string; canceled: boolean; reason?: string; action?: string };
    rejected: Record<string, boolean>;
  };

  expect(result.toast).toEqual({
    id: 'automation-banner-toast',
    canceled: true,
    reason: 'timeout',
  });
  expect(result.prompt).toEqual({
    id: 'automation-banner-prompt',
    canceled: true,
    reason: 'dismissed',
  });
  expect(result.rejected).toEqual({ title: true, emptyId: true, tooMany: true, background: true });
});

autostartSpec('report autostart state and accept an idempotent write', {
  id: 'HOSTAPP-AUTOSTART-001',
  covers: ['lx.app.autostart', 'lx.app.autostart.isEnabled', 'lx.app.autostart.setEnabled'],
  app: SHOWCASE_APP_ID,
  reason: 'Autostart is intentionally absent on mobile hosts.',
  // `SMAppService.mainApp.status` costs ~6s per call on macOS; the spec pays
  // that twice rather than pretending the API is cheap.
  timeout: 90_000,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-AUTOSTART-001');

  // Autostart is a login-item concept the inventory marks optional; a phone
  // host does not build it, and asking is how a caller finds out.
  const offered = await app.eval({
    script: `return !!(lx.app.autostart && typeof lx.app.autostart.isEnabled === 'function')`,
  }) as boolean;
  if (!offered) {
    const supported = await app.eval({ script: `return !!lx.supports({ capability: 'autostart' })` });
    expect(supported).toBe(false);
    return;
  }

  // Writing the value the host already holds proves the setter without
  // registering or removing a real login item on the developer's machine.
  const result = await app.eval({
    // The macOS login-item service answers well past the 5s eval default.
    timeoutMs: 45_000,
    script: `
      const before = await lx.app.autostart.isEnabled();
      await lx.app.autostart.setEnabled(before);
      const after = await lx.app.autostart.isEnabled();
      return { before, after };
    `,
  }) as { before: boolean; after: boolean };

  expect(typeof result.before).toBe('boolean');
  expect(result.after).toBe(result.before);
});

spec('clear product caches while preserving live app storage', {
  id: 'HOSTAPP-CACHE-001',
  covers: ['lx.app.cache', 'lx.app.cache.size', 'lx.app.cache.clear'],
  app: SHOWCASE_APP_ID,
  // A clear walks every lxapp's storage and asks the WebView store to drop its
  // cache; both are slower than a plain property read.
  timeout: 90_000,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-CACHE-001');

  const result = await app.eval({
    timeoutMs: 60_000,
    script: `
      const payload = 'x'.repeat(256 * 1024);
      const cached = lx.env.USER_CACHE_PATH + '/automation/cache-probe.txt';
      const durable = 'automation/cache-probe.txt';
      await lx.fs.write(cached, payload, { overwrite: true });
      await lx.fs.write(durable, payload, { overwrite: true });

      await lx.getStorage().set('automation-cache-probe', 'keep');
      const before = await lx.app.cache.size();
      const report = await lx.app.cache.clear();
      const after = await lx.app.cache.size();

      const cachedSurvived = await lx.fs.exists(cached);
      const durableSurvived = await lx.fs.exists(durable);
      const kv = await lx.getStorage().get('automation-cache-probe');
      await lx.getStorage().delete('automation-cache-probe');
      await lx.fs.remove(durable);
      await lx.fs.remove(cached);

      return { before, report, after, cachedSurvived, durableSurvived, kv };
    `,
  }) as {
    kv: unknown;
    before: number;
    report: { freedBytes: number; skippedActivePaths: number; webview: string; failures: string[] };
    after: number;
    cachedSurvived: boolean;
    durableSurvived: boolean;
  };

  expect(result.before).toBeGreaterThanOrEqual(0);
  expect(result.report.freedBytes).toBeGreaterThanOrEqual(0);
  expect(result.report.skippedActivePaths).toBeGreaterThanOrEqual(1);
  expect(['cleared', 'unsupported', 'failed']).toContain(result.report.webview);
  expect(result.after).toBeGreaterThanOrEqual(0);
  expect(result.cachedSurvived).toBe(true);
  expect(result.durableSurvived).toBe(true);
  expect(result.kv).toBe('keep');
});

spec('show, label, and retract the host tray item', {
  id: 'HOSTAPP-TRAY-001',
  covers: [
    'lx.tray',
    'lx.tray.show',
    'lx.tray.hide',
    'lx.tray.setTitle',
    'lx.tray.setBadge',
    'lx.tray.setMenu',
    'lx.tray.setIcon',
    'lx.tray.onClick',
  ],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-TRAY-001');
  t.defer(async () => {
    await app.eval({ script: `try { lx.tray.hide(); } catch {} return true;` });
  });

  const result = await app.eval({
    script: `
      lx.tray.show();
      lx.tray.setTitle('LX');
      lx.tray.setIcon('public/showcase-icon.svg');
      lx.tray.setBadge('3');
      lx.tray.setMenu([
        { label: 'Open Showcase', onClick: () => {} },
        { separator: true },
        { label: 'Disabled', enabled: false },
      ]);
      const off = lx.tray.onClick(() => {});
      off();
      lx.tray.setBadge(null);
      lx.tray.setTitle(null);
      lx.tray.hide();
      // Hiding a hidden tray stays a no-op.
      lx.tray.hide();
      return { unsubscribe: typeof off };
    `,
  }) as { unsubscribe: string };

  expect(result.unsubscribe).toBe('function');
});

spec('declare, patch, and retract runtime sidebar actions atomically', {
  id: 'HOSTAPP-SIDEBAR-001',
  covers: [
    'lx.shell',
    'lx.shell.sidebarActions',
    'lx.shell.sidebarActions.replace',
    'lx.shell.sidebarActions.update',
    'lx.shell.sidebarActions.remove',
    'lx.shell.sidebarActions.clear',
  ],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-SIDEBAR-001');
  // The declaration is process-local shell chrome; drop it however the spec ends.
  t.defer(async () => {
    await app.eval({
      script: `
        try { lx.shell.sidebarActions.clear(); } catch {}
        if (globalThis.__sidebarProbeIcon) {
          try { await lx.fs.remove(globalThis.__sidebarProbeIcon); } catch {}
          delete globalThis.__sidebarProbeIcon;
        }
        return true;
      `,
    });
  });

  await t.step('declare a header and a footer action', async () => {
    const result = await evalCaught(app, `
      lx.shell.sidebarActions.replace([
        { id: 'probe-header', placement: 'header', icon: 'public/showcase-icon.svg', label: 'Probe header', onActivate() {} },
        { id: 'probe-footer', placement: 'footer', icon: 'public/showcase-icon.svg', label: 'Probe footer', onActivate() {} },
      ]);
      return 'declared';
    `);
    expect(result.ok).toBeTruthy();
  });

  await t.step('patch presentation of one live id', async () => {
    const result = await evalCaught(app, `
      lx.shell.sidebarActions.update('probe-footer', { label: 'Probe footer 2', disabled: true });
      return 'patched';
    `);
    expect(result.ok).toBeTruthy();
  });

  await t.step('accept a runtime-managed raster icon', async () => {
    const result = await evalCaught(app, `
      const shot = await lx.app.screenshot();
      globalThis.__sidebarProbeIcon = shot.tempFilePath;
      lx.shell.sidebarActions.update('probe-footer', { icon: shot.tempFilePath });
      return shot.tempFilePath;
    `);
    expect(result.ok).toBeTruthy();
    expect(result.value).toMatch(/^lx:\/\//);
  });

  await t.step('keep settings-shaped declarations on the generic runtime channel', async () => {
    const result = await evalCaught(app, `
      lx.shell.sidebarActions.replace([
        { id: 'settings', placement: 'footer', icon: 'public/showcase-icon.svg', label: 'ID spoof', onActivate() {} },
        { id: 'label-spoof', placement: 'footer', icon: 'public/showcase-icon.svg', label: 'Settings', onActivate() {} },
        { id: 'icon-spoof', placement: 'footer', icon: 'public/sidebar-settings.svg', label: 'Icon spoof', onActivate() {} },
      ]);
      lx.shell.sidebarActions.update('settings', { label: 'ID spoof live' });
      lx.shell.sidebarActions.update('label-spoof', { disabled: true });
      lx.shell.sidebarActions.update('icon-spoof', { label: 'Icon spoof live' });
      return 'generic';
    `);
    expect(result.ok).toBeTruthy();

    // Restore the live set used by the following rollback assertions.
    const restored = await evalCaught(app, `
      lx.shell.sidebarActions.replace([
        { id: 'probe-header', placement: 'header', icon: 'public/showcase-icon.svg', label: 'Probe header', onActivate() {} },
        { id: 'probe-footer', placement: 'footer', icon: 'public/showcase-icon.svg', label: 'Probe footer 2', disabled: true, onActivate() {} },
      ]);
      return 'restored';
    `);
    expect(restored.ok).toBeTruthy();
  });

  await t.step('reject a patch for an id outside the declaration', async () => {
    const result = await evalCaught(app, `
      lx.shell.sidebarActions.update('probe-missing', { label: 'nope' });
      return 'patched';
    `);
    expect(result.ok).toBeFalsy();
    expect(result.code).toBe('E_NOT_FOUND');
  });

  await t.step('reject a third header action without disturbing the live set', async () => {
    const result = await evalCaught(app, `
      lx.shell.sidebarActions.replace([
        { id: 'h1', placement: 'header', icon: 'public/showcase-icon.svg', label: 'One', onActivate() {} },
        { id: 'h2', placement: 'header', icon: 'public/showcase-icon.svg', label: 'Two', onActivate() {} },
        { id: 'h3', placement: 'header', icon: 'public/showcase-icon.svg', label: 'Three', onActivate() {} },
      ]);
      return 'declared';
    `);
    expect(result.ok).toBeFalsy();
    // The rejected generation must not have replaced the live one.
    const survivor = await evalCaught(app, `
      lx.shell.sidebarActions.update('probe-header', { label: 'Probe header 2' });
      return 'patched';
    `);
    expect(survivor.ok).toBeTruthy();
  });

  await t.step('remove one id, then clear the rest idempotently', async () => {
    const removed = await evalCaught(app, `lx.shell.sidebarActions.remove('probe-footer'); return 'removed';`);
    expect(removed.ok).toBeTruthy();

    const removedTwice = await evalCaught(app, `lx.shell.sidebarActions.remove('probe-footer'); return 'removed';`);
    expect(removedTwice.ok).toBeFalsy();
    expect(removedTwice.code).toBe('E_NOT_FOUND');

    const cleared = await evalCaught(app, `lx.shell.sidebarActions.clear(); lx.shell.sidebarActions.clear(); return 'cleared';`);
    expect(cleared.ok).toBeTruthy();
  });
});

spec('reject shell surface reconfigure for an id the shell never realized', {
  id: 'HOSTAPP-SHELL-RECONFIG-001',
  covers: ['lx.shell.reconfigure'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-SHELL-RECONFIG-001');

  const result = await evalCaught(app, `
    await lx.shell.reconfigure('surface-that-was-never-opened', { as: 'aside', edge: 'trailing' });
    return 'reconfigured';
  `);

  expect(result.ok).toBeFalsy();
  expect(typeof result.code).toBe('string');
  expect(String(result.code).length).toBeGreaterThan(0);
});

spec('subscribe to and release the surface context listener', {
  id: 'HOSTAPP-SURFACE-CTX-001',
  covers: ['lx.surface', 'lx.surface.onContext'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-SURFACE-CTX-001');

  const result = await app.eval({
    script: `
      const first = lx.surface.onContext(() => {});
      const second = lx.surface.onContext(() => {});
      first();
      second();
      // Releasing twice must stay inert rather than throwing.
      first();
      return { kinds: [typeof first, typeof second], distinct: first !== second };
    `,
  }) as { kinds: string[]; distinct: boolean };

  expect(result.kinds).toEqual(['function', 'function']);
  expect(result.distinct).toBeTruthy();
});
