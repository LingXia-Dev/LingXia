import { expect, spec } from '@lingxia/test';
import { bindFixture } from '../helpers/poll.js';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { runtimePlatform } from '../helpers/platform.js';

spec('round-trip text, empty clipboard, and typed image items', {
  id: 'LOGIC-CLIPBOARD-001',
  covers: [
    'lx.clipboard.writeText',
    'lx.clipboard.readText',
    'lx.clipboard.write',
    'lx.clipboard.read',
    'lx.clipboard.clear',
    'lx.clipboard.types',
  ],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app, namespace } = bindFixture(t, 'LOGIC-CLIPBOARD-001');
  const marker = `lx-clipboard-${namespace}`;
  if (await runtimePlatform(app) === 'harmony') {
    await expectHarmonyReadsDenied(app, marker, namespace);
    return;
  }

  const result = await app.eval({
    script: `
      const files = lx.fs;
      const root = lx.env.USER_CACHE_PATH + '/' + ${JSON.stringify(namespace)};
      const fixture = root + '/clipboard.png';
      const png = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==';
      await files.mkdir(root, { recursive: true });
      try {
        await lx.clipboard.writeText(${JSON.stringify(marker)});
        const written = await lx.clipboard.readText();
        const afterWrite = await lx.clipboard.types();
        const typed = await lx.clipboard.read({ type: 'text' });

        await lx.clipboard.write({ type: 'text', text: '' });
        const emptyString = await lx.clipboard.readText();

        await files.write(fixture, png, { encoding: 'base64' });
        await lx.clipboard.write({ type: 'image', filePath: fixture });
        const imageTypes = await lx.clipboard.types();
        const imageRead = await lx.clipboard.read({ type: 'image' });
        const imageItem = (imageRead.status === 'empty') ? null : imageRead.items.find((item) => item.type === 'image');
        const imageInfo = imageItem
          ? await lx.getImageInfo({ path: imageItem.filePath })
          : null;

        await lx.clipboard.clear();
        const afterClear = await lx.clipboard.readText();
        const emptyRead = await lx.clipboard.read();
        const emptyTypes = await lx.clipboard.types();

        return {
          writtenCanceled: (written.status === 'canceled'),
          writtenEmpty: (written.status === 'canceled') ? null : (written.status === 'empty'),
          writtenText: written.status !== 'canceled' && !(written.status === 'empty') ? written.text : null,
          afterWriteTypes: afterWrite,
          typedEmpty: (typed.status === 'canceled') ? true : (typed.status === 'empty'),
          typedText: typed.status !== 'canceled' && !(typed.status === 'empty')
            ? typed.items.find((item) => item.type === 'text')?.text
            : null,
          emptyStringEmpty: (emptyString.status === 'canceled') ? null : (emptyString.status === 'empty'),
          emptyStringText: emptyString.status !== 'canceled' && !(emptyString.status === 'empty') ? emptyString.text : null,
          imageTypes,
          imageEmpty: (imageRead.status === 'canceled') ? true : (imageRead.status === 'empty'),
          imageWidth: imageInfo && imageInfo.width,
          imageHeight: imageInfo && imageInfo.height,
          afterClearEmpty: (afterClear.status === 'canceled') ? null : (afterClear.status === 'empty'),
          emptyRead: (emptyRead.status === 'canceled') ? false : (emptyRead.status === 'empty'),
          emptyTypes,
        };
      } finally {
        await files.remove(root, { recursive: true }).catch(() => {});
      }
    `,
  }) as {
    writtenCanceled: boolean;
    writtenEmpty: boolean | null;
    writtenText: string | null;
    afterWriteTypes: string[];
    typedEmpty: boolean;
    typedText: string | null;
    emptyStringEmpty: boolean | null;
    emptyStringText: string | null;
    imageTypes: string[];
    imageEmpty: boolean;
    imageWidth: number | null;
    imageHeight: number | null;
    afterClearEmpty: boolean | null;
    emptyRead: boolean;
    emptyTypes: string[];
  };

  expect(result.writtenCanceled).toBe(false);
  expect(result.writtenEmpty).toBe(false);
  expect(result.writtenText).toBe(marker);
  expect(result.afterWriteTypes).toContain('text');
  expect(result.typedEmpty).toBe(false);
  expect(result.typedText).toBe(marker);
  expect(result.emptyStringEmpty).toBe(false);
  expect(result.emptyStringText).toBe('');
  expect(result.imageTypes).toContain('image');
  expect(result.imageEmpty).toBe(false);
  expect(result.imageWidth).toBe(1);
  expect(result.imageHeight).toBe(1);
  expect(result.afterClearEmpty).toBe(true);
  expect(result.emptyRead).toBe(true);
  expect(result.emptyTypes).toEqual([]);
});

/**
 * HarmonyOS reads need `ohos.permission.READ_PASTEBOARD`, an ACL permission an
 * ordinary app — Showcase included — does not hold. Everything that needs no
 * permission still works, and a read is a clear permission rejection rather
 * than an empty clipboard, even while the permission-free peek sees text.
 */
async function expectHarmonyReadsDenied(
  app: Parameters<typeof runtimePlatform>[0],
  marker: string,
  namespace: string,
): Promise<void> {
  const result = await app.eval({
    script: `
      const files = lx.fs;
      const root = lx.env.USER_CACHE_PATH + '/' + ${JSON.stringify(namespace)};
      const fixture = root + '/clipboard.png';
      const png = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==';
      const rejection = async (call) => {
        try {
          await call();
          return 'resolved';
        } catch (error) {
          return String(error?.code ?? error?.data?.code ?? error);
        }
      };
      await files.mkdir(root, { recursive: true });
      try {
        await lx.clipboard.writeText(${JSON.stringify(marker)});
        const textTypes = await lx.clipboard.types();
        const readText = await rejection(() => lx.clipboard.readText());
        const read = await rejection(() => lx.clipboard.read({ type: 'text' }));

        await files.write(fixture, png, { encoding: 'base64' });
        await lx.clipboard.write({ type: 'image', filePath: fixture });
        const imageTypes = await lx.clipboard.types();
        const readImage = await rejection(() => lx.clipboard.read({ type: 'image' }));

        await lx.clipboard.clear();
        const afterClear = await lx.clipboard.readText();
        const emptyTypes = await lx.clipboard.types();
        return {
          textTypes,
          readText,
          read,
          imageTypes,
          readImage,
          afterClearEmpty: (afterClear.status === 'canceled') ? null : (afterClear.status === 'empty'),
          emptyTypes,
        };
      } finally {
        await files.remove(root, { recursive: true }).catch(() => {});
      }
    `,
  }) as {
    textTypes: string[];
    readText: string;
    read: string;
    imageTypes: string[];
    readImage: string;
    afterClearEmpty: boolean | null;
    emptyTypes: string[];
  };

  expect(result.textTypes).toContain('text');
  expect(result.readText).toBe('E_PERMISSION_DENIED');
  expect(result.read).toBe('E_PERMISSION_DENIED');
  expect(result.imageTypes).toContain('image');
  expect(result.readImage).toBe('E_PERMISSION_DENIED');
  // An empty clipboard is answered before any read, so it needs no permission.
  expect(result.afterClearEmpty).toBe(true);
  expect(result.emptyTypes).toEqual([]);
}
