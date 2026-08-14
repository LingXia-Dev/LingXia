import { expect } from '@rongjs/test';
import { contract } from '../support/contract.js';

contract({
  id: 'LOGIC-005',
  title: 'reject storage and file operations on invalid inputs',
  covers: ['Storage.set', 'LxFile.text', 'FileSystemApi.stat'],
  layer: 'logic',
  levels: ['failure', 'boundary'],
  scope: 'portable',
  expectedOutcome: 'reject',
}, async ({ app, namespace }) => {
  const result = await app.eval({
    script: `
      const files = lx.fs;
      const storage = lx.getStorage();
      const missing = lx.env.USER_CACHE_PATH + '/' + ${JSON.stringify(namespace)} + '/missing.txt';
      const rejects = async (operation) => {
        try {
          await operation();
          return false;
        } catch {
          return true;
        }
      };
      const oversized = 'x'.repeat(5 * 1024 * 1024 + 1);
      return {
        readMissing: await rejects(() => files.file(missing).text()),
        statMissing: await rejects(() => files.stat(missing)),
        oversizedValue: await rejects(() => storage.set(${JSON.stringify(namespace)} + '-oversized', oversized)),
      };
    `,
  }) as {
    readMissing: boolean;
    statMissing: boolean;
    oversizedValue: boolean;
  };

  expect(result.readMissing).toBeTruthy();
  expect(result.statMissing).toBeTruthy();
  expect(result.oversizedValue).toBeTruthy();
});

contract({
  id: 'MEDIA-INFO-001',
  title: 'read image info from managed storage and reject a missing image',
  covers: ['lx.getImageInfo'],
  layer: 'logic',
  levels: ['semantic', 'failure'],
  scope: 'portable',
  expectedOutcome: 'supported',
}, async ({ app, namespace }) => {
  const result = await app.eval({
    script: `
      const files = lx.fs;
      const root = lx.env.USER_CACHE_PATH + '/' + ${JSON.stringify(namespace)};
      const fixture = root + '/fixture.png';
      // 1x1 red pixel PNG.
      const base64 = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==';
      const rejects = async (operation) => {
        try {
          await operation();
          return false;
        } catch {
          return true;
        }
      };
      await files.mkdir(root, { recursive: true });
      try {
        await files.write(fixture, base64, { encoding: 'base64' });
        const info = await lx.getImageInfo({ path: fixture });
        const missingRejected = await rejects(() => lx.getImageInfo({ path: root + '/missing.png' }));
        return { width: info.width, height: info.height, type: info.type, missingRejected };
      } finally {
        await files.remove(root, { recursive: true });
      }
    `,
  }) as {
    width: number;
    height: number;
    type: string;
    missingRejected: boolean;
  };

  expect(result.width).toBe(1);
  expect(result.height).toBe(1);
  expect(result.type).toContain('png');
  expect(result.missingRejected).toBeTruthy();
});
