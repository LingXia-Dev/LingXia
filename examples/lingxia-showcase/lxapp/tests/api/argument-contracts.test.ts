import { expect, spec } from '@lingxia/test';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { bindFixture, evalCaught } from '../helpers/poll.js';

/** One rejected call: the expression, and the code an app author branches on. */
interface Rejection {
  readonly label: string;
  readonly call: string;
  readonly code: string;
}

async function assertRejections(
  t: Parameters<Parameters<typeof spec>[2]>[0],
  app: ReturnType<typeof bindFixture>['app'],
  cases: readonly Rejection[],
): Promise<void> {
  const observed: string[] = [];
  for (const item of cases) {
    const outcome = await evalCaught(app, `return await ${item.call};`);
    observed.push(`${item.label}=${outcome.ok ? 'ACCEPTED' : String(outcome.code)}`);
  }
  // Compare the whole table at once: one failing row then names every other
  // code that moved with it, instead of stopping at the first.
  expect(observed).toEqual(cases.map((item) => `${item.label}=${item.code}`));
}

spec('reject malformed navigation arguments with stable codes', {
  id: 'ARGS-NAV-001',
  covers: ['lx.navigateTo', 'lx.redirectTo', 'lx.switchTab'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'ARGS-NAV-001');
  // Whatever route the suite left us on is the one that must survive; naming
  // a page here would make this spec depend on the order it runs in.
  const before = await app.nav.current();

  await assertRejections(t, app, [
    { label: 'navigateTo no page', call: `lx.navigateTo({})`, code: 'E_INVALID_ARG' },
    { label: 'navigateTo empty page', call: `lx.navigateTo({ page: '' })`, code: 'E_INVALID_ARG' },
    { label: 'navigateTo unknown page', call: `lx.navigateTo({ page: 'no-such-page' })`, code: 'E_NOT_FOUND' },
    { label: 'redirectTo unknown page', call: `lx.redirectTo({ page: 'no-such-page' })`, code: 'E_NOT_FOUND' },
    { label: 'switchTab unknown page', call: `lx.switchTab({ page: 'no-such-page' })`, code: 'E_NOT_FOUND' },
  ]);

  // A rejected navigation must not have moved the stack.
  const after = await app.nav.current();
  expect(after.name).toBe(before.name);
  expect(after.path).toBe(before.path);
});

spec('reject malformed transfer arguments before touching the network', {
  id: 'ARGS-TRANSFER-001',
  covers: ['lx.downloadFile', 'lx.uploadFile'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'ARGS-TRANSFER-001');

  await assertRejections(t, app, [
    { label: 'no url', call: `lx.downloadFile({})`, code: 'E_INVALID_ARG' },
    { label: 'empty url', call: `lx.downloadFile({ url: '' })`, code: 'E_INVALID_ARG' },
    // Scheme and host are authority questions, not shape questions.
    { label: 'non-http scheme', call: `lx.downloadFile({ url: 'ftp://example.com/a' })`, code: 'E_PERMISSION_DENIED' },
    { label: 'untrusted host', call: `lx.downloadFile({ url: 'https://not-trusted.example/a' })`, code: 'E_PERMISSION_DENIED' },
    // The Showcase trusts 127.0.0.1 so its suite can reach a fixture server,
    // and a dev session is what unlocks that. Policy therefore lets this
    // through and it fails at connect instead — port 1 answers nothing.
    // A release build has no dev session and denies it outright.
    { label: 'trusted loopback', call: `lx.downloadFile({ url: 'http://127.0.0.1:1/a' })`, code: 'E_NETWORK' },
    // A private address the lxapp never named stays denied even in dev.
    { label: 'untrusted private range', call: `lx.downloadFile({ url: 'https://192.168.0.1/a' })`, code: 'E_PERMISSION_DENIED' },

    // uploadFile rejects on shape before it opens the file, so these rows need
    // no fixture and no source file -- they hold on every platform.
    { label: 'upload no url', call: `lx.uploadFile({})`, code: 'E_INVALID_ARG' },
    { label: 'upload no filePath', call: `lx.uploadFile({ url: 'http://127.0.0.1:1/upload' })`, code: 'E_INVALID_ARG' },
    { label: 'upload empty url', call: `lx.uploadFile({ url: '', filePath: 'a.bin' })`, code: 'E_INVALID_ARG' },
    { label: 'upload unknown method', call: `lx.uploadFile({ url: 'http://127.0.0.1:1/upload', filePath: 'a.bin', method: 'DELETE' })`, code: 'E_INVALID_ARG' },
    { label: 'upload unknown bodyMode', call: `lx.uploadFile({ url: 'http://127.0.0.1:1/upload', filePath: 'a.bin', bodyMode: 'binary' })`, code: 'E_INVALID_ARG' },
    // Multipart-only options under a raw body are rejected, never dropped.
    { label: 'upload raw with formData', call: `lx.uploadFile({ url: 'http://127.0.0.1:1/upload', filePath: 'a.bin', bodyMode: 'raw', formData: { note: 'x' } })`, code: 'E_INVALID_ARG' },
    { label: 'upload raw with name', call: `lx.uploadFile({ url: 'http://127.0.0.1:1/upload', filePath: 'a.bin', bodyMode: 'raw', name: 'asset' })`, code: 'E_INVALID_ARG' },
    // Shape is fine here; the file simply is not there.
    { label: 'upload missing file', call: `lx.uploadFile({ url: 'http://127.0.0.1:1/upload', filePath: 'no-such-file.bin' })`, code: 'E_INVALID_ARG' },
  ]);
});

spec('answer a missing file with not-found, never an internal error', {
  id: 'ARGS-FS-001',
  covers: ['lx.fs.stat', 'lx.fs.exists', 'lx.fs.readDir', 'LxFile.text', 'LxFile.exists'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app, namespace } = bindFixture(t, 'ARGS-FS-001');
  const missing = `lx://userdata/${namespace}/absent.txt`;

  await assertRejections(t, app, [
    // "Does this file exist?" is the most common question asked of the
    // filesystem, and E_INTERNAL is not an answer a caller can act on.
    { label: 'stat missing', call: `lx.fs.stat(${JSON.stringify(missing)})`, code: 'E_NOT_FOUND' },
    { label: 'readDir missing', call: `lx.fs.readDir(${JSON.stringify(missing)})`, code: 'E_NOT_FOUND' },
    { label: 'file text missing', call: `lx.fs.file(${JSON.stringify(missing)}).text()`, code: 'E_NOT_FOUND' },
    { label: 'stat empty path', call: `lx.fs.stat('')`, code: 'E_INVALID_ARG' },
    { label: 'stat native absolute', call: `lx.fs.stat('/etc/passwd')`, code: 'E_INVALID_ARG' },
  ]);

  // exists() answers the same question without throwing at all.
  const exists = await app.eval({
    script: `return [
      await lx.fs.exists(${JSON.stringify(missing)}),
      await lx.fs.file(${JSON.stringify(missing)}).exists(),
    ];`,
  });
  expect(exists).toEqual([false, false]);
});
