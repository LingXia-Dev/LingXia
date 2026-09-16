import { expect, spec } from '@lingxia/test';
import { SHOWCASE_APP_ID } from '../../helpers/app.js';
import { runtimePlatform } from '../../helpers/platform.js';
import { bindFixture } from '../../helpers/poll.js';

/**
 * `lx.clipboard` is the system clipboard, not a runtime-private buffer. The
 * shared round-trip cannot tell the two apart, so this case crosses the
 * boundary with the desktop driver, which reads and writes the OS clipboard
 * through its own native path: text placed by the OS side is what `readText`
 * sees, and what `writeText` / `clear` do is what the OS side sees.
 *
 * The driver runs in the host process, so neither direction raises the macOS
 * paste prompt. The person's clipboard text is put back afterwards.
 */
spec('lx.clipboard reads and writes the same clipboard the OS sees', {
  id: 'DESKTOP-CLIPBOARD-001',
  covers: [
    'lx.clipboard.readText',
    'lx.clipboard.writeText',
    'lx.clipboard.clear',
    'lx.clipboard.types',
    'DesktopClipboard.get',
    'DesktopClipboard.set',
  ],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const auto = lx.automation();
  const { app, namespace } = bindFixture(t, 'DESKTOP-CLIPBOARD-001');
  const platform = await runtimePlatform(app);
  if (platform !== 'macos' && platform !== 'windows') {
    throw new Error(`desktop clipboard case ran against ${platform || 'unknown'}`);
  }
  const doctor = await auto.desktop.doctor();
  if (!doctor.capabilities.clipboard) {
    throw new Error('desktop driver reports no clipboard capability on a desktop host');
  }
  const desktop = auto.desktop.clipboard;

  const previous = (await desktop.get()).text;
  t.defer(async () => {
    if (previous === null) await desktop.clear();
    else await desktop.set({ text: previous });
  });

  const fromOs = `os-to-lx-${namespace}-中文`;
  await desktop.set({ text: fromOs });
  const seen = await app.eval({
    script: `
      const read = await lx.clipboard.readText();
      const types = await lx.clipboard.types();
      return {
        canceled: read.canceled,
        empty: read.canceled ? null : read.empty,
        text: !read.canceled && !read.empty ? read.text : null,
        types: types.canceled ? null : types.types,
      };
    `,
  }) as { canceled: boolean; empty: boolean | null; text: string | null; types: string[] | null };
  expect(seen.canceled).toBe(false);
  expect(seen.empty).toBe(false);
  expect(seen.text).toBe(fromOs);
  expect(seen.types).toContain('text');

  const fromLx = `lx-to-os-${namespace}-emoji-✓`;
  await app.eval({ script: `await lx.clipboard.writeText(${JSON.stringify(fromLx)}); return true;` });
  expect((await desktop.get()).text).toBe(fromLx);

  await app.eval({ script: `await lx.clipboard.clear(); return true;` });
  const afterClear = await desktop.get();
  expect(afterClear.text ?? '').toBe('');
});
