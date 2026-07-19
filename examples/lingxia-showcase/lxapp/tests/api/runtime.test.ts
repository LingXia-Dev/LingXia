import { expect, test } from '@rongjs/test';

test('reads core app, device, screen, network, and system state', async () => {
  const result = await lx.automation().lxapp().eval({
    script: `
      const app = lx.getLxAppInfo();
      const device = lx.getDeviceInfo();
      const screen = lx.getScreenInfo();
      const network = await lx.getNetworkInfo();
      const system = lx.getSystemSetting();
      const host = lx.app.getBaseInfo();
      return {
        appId: app.appId,
        device: !!device.osName,
        screen: screen.width > 0 && screen.height > 0 && screen.scale > 0,
        network: typeof network.isConnected === 'boolean' && !!network.networkType,
        system: typeof system.wifiEnabled === 'boolean',
        host: !!host.os && !!host.productName,
        envVersion: lx.app.envVersion,
      };
    `,
  }) as {
    appId: string;
    device: boolean;
    screen: boolean;
    network: boolean;
    system: boolean;
    host: boolean;
    envVersion: string;
  };

  expect(result.appId).toBe('lingxia-showcase');
  expect(result.device).toBeTruthy();
  expect(result.screen).toBeTruthy();
  expect(result.network).toBeTruthy();
  expect(result.system).toBeTruthy();
  expect(result.host).toBeTruthy();
  expect(['developer', 'preview', 'release']).toContain(result.envVersion);
});

test('registers and removes portable runtime listeners', async () => {
  const result = await lx.automation().lxapp().eval({
    script: `
      const callback = () => {};
      lx.onNetworkChange(callback);
      lx.offNetworkChange(callback);
      lx.onDeviceOrientationChange(callback);
      lx.offDeviceOrientationChange(callback);
      lx.onKeyDown(callback);
      lx.offKeyDown(callback);
      lx.onKeyUp(callback);
      lx.offKeyUp(callback);
      lx.onWifiConnected(callback);
      lx.offWifiConnected(callback);
      return true;
    `,
  });

  expect(result).toBeTruthy();
});

test('round-trips isolated key-value storage', async () => {
  const result = await lx.automation().lxapp().eval({
    script: `
      const storage = lx.getStorage();
      const key = 'automation:' + Date.now();
      const before = await storage.info();
      await storage.set(key, { ok: true, count: 2 });
      const value = await storage.get(key);
      const present = Array.from(await storage.list()).includes(key);
      await storage.delete(key);
      const after = await storage.info();
      return {
        value,
        present,
        removed: !Array.from(await storage.list()).includes(key),
        sizeRestored: after.keyCount === before.keyCount,
      };
    `,
  }) as {
    value: unknown;
    present: boolean;
    removed: boolean;
    sizeRestored: boolean;
  };

  expect(result.value).toEqual({ ok: true, count: 2 });
  expect(result.present).toBeTruthy();
  expect(result.removed).toBeTruthy();
  expect(result.sizeRestored).toBeTruthy();
});

test('round-trips files under lx user cache', async () => {
  const result = await lx.automation().lxapp().eval({
    script: `
      const files = lx.getFileManager();
      const root = lx.env.USER_CACHE_PATH + '/automation-' + Date.now();
      const source = root + '/source.txt';
      const renamed = root + '/renamed.txt';
      const copied = root + '/copied.txt';
      await files.mkdir({ path: root, recursive: true });
      try {
        await files.writeFile({ filePath: source, data: 'hello automation' });
        const text = await files.readFile({ filePath: source, encoding: 'utf8' });
        const stat = await files.stat({ path: source });
        await files.rename({ oldPath: source, newPath: renamed });
        await files.copyFile({ srcPath: renamed, destPath: copied });
        return {
          text: text.data,
          isFile: stat.isFile,
          renamed: await files.exists({ path: renamed }),
          copied: await files.exists({ path: copied }),
        };
      } finally {
        await files.remove({ path: root, recursive: true });
      }
    `,
    timeoutMs: 15_000,
  }) as {
    text: string;
    isFile: boolean;
    renamed: boolean;
    copied: boolean;
  };

  expect(result.text).toBe('hello automation');
  expect(result.isFile).toBeTruthy();
  expect(result.renamed).toBeTruthy();
  expect(result.copied).toBeTruthy();
});
