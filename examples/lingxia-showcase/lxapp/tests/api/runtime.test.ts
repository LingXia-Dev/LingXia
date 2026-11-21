import { expect, test } from '@lingxia/test';
import { bindFixture, type Caught } from '@lingxia/test/showcase/helpers/poll.js';
import { waitForCurrentPage } from '@lingxia/test/showcase/helpers/page.js';
import { SHOWCASE_APP_ID } from '@lingxia/test/showcase/helpers/app.js';


test("read core app, device, screen, network, and system state", { id: "LOGIC-001", covers: ['lx.app.info', 'lx.app.info.role', 'lx.device.info', 'lx.device.info.osName', 'lx.device.screen', 'lx.network.status', 'lx.device.systemSettings', 'lx.host.info', 'lx.host.env'], app: SHOWCASE_APP_ID }, async (t) => {
  const { app } = bindFixture(t, "LOGIC-001");

  const result = await app.logic.eval(async ({ lx }) => {
    const app = lx.app.info;
    const device = lx.device.info;
    const screen = lx.device.screen();
    const network = await lx.network.status();
    const system = lx.device.systemSettings();
    const host = lx.host.info;
    return {
      appId: app.appId,
      role: app.role,
      device: !!device.osName,
      screen: screen.width > 0 && screen.height > 0 && screen.scale > 0,
      network: typeof network.isConnected === 'boolean' && !!network.networkType,
      system: typeof system.wifiEnabled === 'boolean',
      host: !!host.os && !!host.productName,
      env: lx.host.env,
    };
  });

  expect(result.appId).toBe(SHOWCASE_APP_ID);
  expect(['control', 'control-surface', 'guest']).toContain(result.role);
  expect(result.device).toBeTruthy();
  expect(result.screen).toBeTruthy();
  expect(result.network).toBeTruthy();
  expect(result.system).toBeTruthy();
  expect(result.host).toBeTruthy();
  expect(['dev', 'prod']).toContain(result.env);
});

test("register and remove portable runtime listeners", { id: "LOGIC-002", covers: ['lx.network.watch', 'lx.device.orientation.watch', 'lx.device.onKeyDown', 'lx.device.onKeyUp', 'lx.network.wifi.onConnected'], app: SHOWCASE_APP_ID }, async (t) => {
  const { app } = bindFixture(t, "LOGIC-002");

  const result = await app.logic.eval(async ({ lx }) => {
    const callback = () => {};
    const unsubscribes = [
      lx.network.watch(callback),
      lx.device.orientation.watch(callback),
      lx.device.onKeyDown(callback),
      lx.device.onKeyUp(callback),
      lx.network.wifi.onConnected(callback),
    ];
    if (unsubscribes.some((off) => typeof off !== 'function')) return false;
    unsubscribes.forEach((off) => off());
    // A spent handle is inert: calling it again must not disturb a later
    // subscription on the same function.
    const offAgain = lx.device.orientation.watch(callback);
    unsubscribes.forEach((off) => off());
    offAgain();
    return true;
  });

  expect(result).toBeTruthy();
});

test("answer capability questions and refuse what the host lacks", { id: "LOGIC-006", covers: ['lx.supports'], app: SHOWCASE_APP_ID }, async (t) => {
  const { app } = bindFixture(t, "LOGIC-006");

  const result = await app.logic.eval(async ({ lx }) => {
    // Rule 2: a member is always present; without the feature its call rejects
    // E_NOT_SUPPORTED, without the right E_PERMISSION_DENIED.
    const outcome = (call: () => Promise<unknown>) =>
      call().then(() => 'ok', (error: { code?: string }) => error?.code ?? String(error));
    // Keys and values outside `LxFeature` on purpose: the contract is how it answers them.
    const supports = (value: unknown) => (lx.supports as (value: unknown) => boolean)(value);
    const rejects = (value: unknown) => {
      try { supports(value); return false; }
      catch (error) { return error instanceof TypeError; }
    };
    return {
      os: lx.host.info.os,
      role: lx.app.info.role,
      supported: {
        terminal: lx.supports('terminal'),
        autostart: lx.supports('app.autostart'),
        notification: lx.supports('app.notification'),
        banner: lx.supports('app.banner'),
      },
      calls: {
        terminal: await outcome(() => lx.host.terminal.settings.get()),
        autostart: await outcome(() => lx.host.autostart.isEnabled()),
        notification: await outcome(() => lx.host.notification.getPermission()),
        banner: await outcome(() => lx.host.banner.dismiss('lx-capability-probe')),
      },
      cache: await outcome(() => lx.host.cache.size()),
      unknownFalse: ['', 'future.feature', 'not a key', 'surface.aside', 'control',
        'app.cache', 'surface.main', 'surface.float']
        .every(key => supports(key) === false),
      typeErrors: [{}, null, 42, undefined, { capability: 'terminal' }].every(rejects),
      dependency: !lx.supports('surface.window.fullChrome') || lx.supports('surface.window'),
    };
  });

  const desktop = result.os === 'macOS' || result.os === 'Windows';
  // The showcase host declares notifications and autostart; autostart and
  // banners are desktop-only; the terminal belongs to the terminal app's context.
  expect(result.supported).toEqual({ terminal: false, autostart: desktop, notification: true, banner: desktop });
  for (const [feature, supported] of Object.entries(result.supported)) {
    expect({ feature, outcome: result.calls[feature as keyof typeof result.calls] })
      .toEqual({ feature, outcome: supported ? 'ok' : 'E_NOT_SUPPORTED' });
  }
  expect(result.cache).toBe(result.role === 'control' ? 'ok' : 'E_PERMISSION_DENIED');
  expect(result.unknownFalse).toBe(true);
  expect(result.typeErrors).toBe(true);
  expect(result.dependency).toBe(true);
});

test("round-trip isolated key-value storage", { id: "LOGIC-003", covers: ['lx.storage', 'Storage.info', 'Storage.set', 'Storage.get', 'Storage.has', 'Storage.list', 'Storage.delete'], app: SHOWCASE_APP_ID }, async (t) => {
  const { app, namespace } = bindFixture(t, "LOGIC-003");

  const result = await app.logic.eval(async ({ lx }, namespace) => {
    const storage = lx.storage();
    const key = namespace;
    const before = await storage.info();
    const absentBefore = await storage.has(key);
    let value;
    let present = false;
    let hasWhilePresent = false;
    try {
      await storage.set(key, { ok: true, count: 2 });
      value = await storage.get(key);
      present = (await storage.list()).includes(key);
      hasWhilePresent = await storage.has(key);
    } finally {
      await storage.delete(key);
    }
    const after = await storage.info();
    return {
      value,
      present,
      hasWhilePresent,
      absentBefore,
      hasAfterDelete: await storage.has(key),
      removed: !(await storage.list()).includes(key),
      sizeRestored: after.keyCount === before.keyCount,
    };
  }, namespace);

  expect(result.value).toEqual({ ok: true, count: 2 });
  expect(result.present).toBeTruthy();
  // `has` must agree with `list` while the key is stored, and must report a
  // boolean rather than a value on both sides of the key's lifetime.
  expect(result.hasWhilePresent).toBeTruthy();
  expect(result.absentBefore).toBeFalsy();
  expect(result.hasAfterDelete).toBeFalsy();
  expect(result.removed).toBeTruthy();
  expect(result.sizeRestored).toBeTruthy();
});

test("round-trip files under lx user cache", { id: "LOGIC-004", covers: ['lx.fs', 'lx.fs.copy', 'lx.fs.exists', 'lx.fs.file', 'lx.fs.mkdir', 'lx.fs.remove', 'lx.fs.rename', 'lx.fs.write', 'lx.fs.userCache', 'lx.fs.userData', 'LxFile.text', 'LxFile.json', 'LxFile.base64', 'LxFile.bytes', 'LxFile.arrayBuffer', 'LxFile.stat'], app: SHOWCASE_APP_ID }, async (t) => {
  const { app, namespace } = bindFixture(t, "LOGIC-004");

  const result = await app.logic.eval({ timeout: 15_000 }, async ({ lx }, namespace) => {
    const files = lx.fs;
    const root = lx.fs.userCache + '/' + namespace;
    const dataPathAvailable = typeof lx.fs.userData === 'string' && lx.fs.userData.length > 0;
    const source = root + '/source.txt';
    const renamed = root + '/renamed.txt';
    const copied = root + '/copied.txt';
    await files.mkdir(root, { recursive: true });
    try {
      const payload = JSON.stringify({ hello: 'automation' });
      await files.write(source, { data: payload });
      const file = files.file(source);
      const text = await file.text();
      const json = await file.json();
      const bytes = await file.bytes();
      const buffer = await file.arrayBuffer();
      const base64 = await file.base64();
      const stat = await file.stat();
      await files.rename(source, { to: renamed });
      await files.copy(renamed, { to: copied });
      return {
        text,
        json,
        byteLength: bytes.byteLength,
        arrayBufferLength: buffer.byteLength,
        isUint8Array: bytes instanceof Uint8Array,
        base64,
        isFile: stat.isFile,
        renamed: await files.exists(renamed),
        copied: await files.exists(copied),
        dataPathAvailable,
      };
    } finally {
      await files.remove(root, { recursive: true });
    }
  }, namespace);

  expect(result.text).toBe('{"hello":"automation"}');
  expect(result.json).toEqual({ hello: 'automation' });
  expect(result.byteLength).toBe(result.text.length);
  expect(result.arrayBufferLength).toBe(result.text.length);
  expect(result.isUint8Array).toBeTruthy();
  expect(result.base64).toBe('eyJoZWxsbyI6ImF1dG9tYXRpb24ifQ==');
  expect(result.isFile).toBeTruthy();
  expect(result.renamed).toBeTruthy();
  expect(result.copied).toBeTruthy();
  expect(result.dataPathAvailable).toBeTruthy();
});

test("clear, prefix-list, missing vs null, and persist storage across reLaunch", {
  id: "LOGIC-007",
  covers: ['Storage.clear', 'Storage.list', 'Storage.get', 'Storage.set', 'lx.nav.reset'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app, namespace, defer } = bindFixture(t, "LOGIC-007");
  const persistKey = `${namespace}-persist`;
  defer(async () => {
    await app.logic.eval(async ({ lx }, namespace) => {
      const storage = lx.storage();
      const keys = await storage.list(namespace);
      for (const key of keys) await storage.delete(key);
    }, namespace).catch(() => undefined);
  });

  const first = await app.logic.eval(async ({ lx }, namespace, persistKey) => {
    const storage = lx.storage();
    const ns = namespace;
    await storage.set(ns + '-a', 1);
    await storage.set(ns + '-b', 2);
    await storage.set(ns + '-null', null);
    const prefixed = (await storage.list(ns)).sort();
    const storedNull = await storage.get(ns + '-null');
    const missing = await storage.get(ns + '-missing');
    const listedNull = prefixed.includes(ns + '-null');
    const listedMissing = prefixed.includes(ns + '-missing');
    await storage.set(persistKey, { kept: true });
    await storage.clear();
    const afterClear = await storage.list(ns);
    const persistGone = await storage.get(persistKey);
    await storage.set(persistKey, { kept: true });
    // A missing key reads as undefined, which a JSON result cannot carry.
    return { prefixed, storedNull, missing: missing ?? null, listedNull, listedMissing, afterClear, persistGone: persistGone ?? null };
  }, namespace, persistKey);

  expect(first.prefixed).toEqual([`${namespace}-a`, `${namespace}-b`, `${namespace}-null`].sort());
  expect(first.storedNull).toBe(null);
  expect(first.listedNull).toBeTruthy();
  expect(first.listedMissing).toBeFalsy();
  expect(first.missing).toBe(null);
  expect(first.afterClear).toEqual([]);
  expect(first.persistGone).toBe(null);

  await app.nav.reset({ page: 'home' });
  await waitForCurrentPage(app, 'home', 30_000);
  const persisted = await app.logic.eval(async ({ lx }, persistKey) => await lx.storage().get(persistKey), persistKey);
  expect(persisted).toEqual({ kept: true });

  const oversized: Caught = await app.logic.eval(async ({ lx }, namespace) => {
    try {
      await lx.storage().set(namespace + '-big', 'x'.repeat(5 * 1024 * 1024 + 1));
      return { ok: true };
    } catch (error) {
      const { code, message } = error as { code?: string; message?: string };
      return { ok: false, code: code ?? '', message: String(message ?? error) };
    }
  }, namespace);
  expect(oversized.ok).toBeFalsy();
  expect(oversized.code).toBe('E_OUT_OF_RANGE');
  const leftover = await app.logic.eval(async ({ lx }, namespace) => await lx.storage().get(namespace + '-big'), namespace);
  expect(leftover == null).toBeTruthy();
});

test("replace on write, read directories, LxFile.exists/path, and deny escaped paths", {
  id: "LOGIC-008",
  covers: ['lx.fs.write', 'lx.fs.readDir', 'lx.fs.stat', 'LxFile.exists', 'LxFile.path'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app, namespace } = bindFixture(t, "LOGIC-008");

  const result = await app.logic.eval(async ({ lx }, namespace) => {
    const files = lx.fs;
    const root = lx.fs.userCache + '/' + namespace;
    const child = root + '/child.txt';
    await files.mkdir(root, { recursive: true });
    try {
      await files.write(child, { data: 'hi' });
      await files.write(child, { data: 'overwritten' });
      let overwriteRejected = false;
      try {
        await files.write(child, { data: 'nope', overwrite: false });
      } catch {
        overwriteRejected = true;
      }
      const entries = await files.readDir(root);
      const file = files.file(child);
      const exists = await file.exists();
      const text = await file.text();
      const stat = await files.stat(root);
      return {
        names: entries.map((entry) => entry.name).sort(),
        childIsFile: entries.some((entry) => entry.name === 'child.txt' && entry.isFile),
        // Entries must be plain data: a class wrapper serializes to null.
        serializedIsFile: (JSON.parse(JSON.stringify(entries)) as typeof entries)
          .some((entry) => entry.name === 'child.txt' && entry.isFile),
        exists,
        path: file.path,
        text,
        dirStat: stat.isDirectory,
        overwriteRejected,
      };
    } finally {
      await files.remove(root, { recursive: true });
    }
  }, namespace);

  expect(result.childIsFile).toBeTruthy();
  expect(result.serializedIsFile).toBeTruthy();
  expect(result.exists).toBeTruthy();
  expect(result.path).toContain(namespace);
  expect(result.text).toBe('overwritten');
  expect(result.dirStat).toBeTruthy();
  expect(result.overwriteRejected).toBeTruthy();

  const statCaught = (path: string) => app.logic.eval(async ({ lx }, path): Promise<Caught> => {
    try {
      await lx.fs.stat(path);
      return { ok: true };
    } catch (error) {
      const { code, message } = error as { code?: string; message?: string };
      return { ok: false, code: code ?? '', message: String(message ?? error) };
    }
  }, path);
  const escaped = await statCaught('../secret');
  expect(escaped.ok).toBeFalsy();
  expect(escaped.code).toBe('E_INVALID_ARG');
  const absolute = await statCaught('C:/Windows/System32');
  expect(absolute.ok).toBeFalsy();
  expect(absolute.code).toBe('E_INVALID_ARG');
});
