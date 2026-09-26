import { expect, spec, type Fixture, type JsonValue, type TestApp } from '@lingxia/test';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { bindFixture, type Caught } from '../helpers/poll.js';

/**
 * One rejected call and the code an app author branches on: `api` is the
 * `lx` member path, called with `args` (arguments the typings refuse on
 * purpose), then `then` read (or called, when it is a method) off its result.
 */
interface Rejection {
  readonly label: string;
  readonly api: string;
  readonly args: JsonValue[];
  readonly then?: string;
  readonly code: string;
}

/** Call `lx.<api>(...args)` in Logic and settle it, with `then` read or called off the result. */
async function settle(app: TestApp, item: Rejection): Promise<Caught> {
  return app.logic.eval(async ({ lx }, api, args, then) => {
    try {
      const path = api.split('.');
      let owner: unknown = lx;
      for (const key of path.slice(0, -1)) owner = (owner as Record<string, unknown>)[key];
      const call = (owner as Record<string, (...values: unknown[]) => unknown>)[path[path.length - 1]];
      let value: unknown = await call.apply(owner, args);
      if (then !== null) {
        const member = (value as Record<string, unknown>)[then];
        value = await (typeof member === 'function' ? (member as () => unknown).call(value) : member);
      }
      return { ok: true } as const;
    } catch (error) {
      const { code, message } = error as { code?: string; message?: string };
      return { ok: false, code, message: String(message ?? error) } as const;
    }
  }, item.api, item.args, item.then ?? null);
}

async function assertRejections(
  _t: Fixture,
  app: TestApp,
  cases: readonly Rejection[],
): Promise<void> {
  const observed: string[] = [];
  for (const item of cases) {
    const outcome = await settle(app, item);
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
    { label: 'navigateTo no page', api: 'navigateTo', args: [{}], code: 'E_INVALID_ARG' },
    { label: 'navigateTo empty page', api: 'navigateTo', args: [{ page: '' }], code: 'E_INVALID_ARG' },
    { label: 'navigateTo unknown page', api: 'navigateTo', args: [{ page: 'no-such-page' }], code: 'E_NOT_FOUND' },
    { label: 'redirectTo unknown page', api: 'redirectTo', args: [{ page: 'no-such-page' }], code: 'E_NOT_FOUND' },
    { label: 'switchTab unknown page', api: 'switchTab', args: [{ page: 'no-such-page' }], code: 'E_NOT_FOUND' },
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
    { label: 'no url', api: 'downloadFile', args: [{}], code: 'E_INVALID_ARG' },
    { label: 'empty url', api: 'downloadFile', args: [{ url: '' }], code: 'E_INVALID_ARG' },
    // Home has no provider, so public hosts are allowed. ftp is not a grant
    // question; the transfer client rejects the scheme as a network error.
    // Transfer-time failures surface on `result`, not from the factory call.
    { label: 'non-http scheme', api: 'downloadFile', args: [{ url: 'ftp://example.com/a' }], then: 'result', code: 'E_NETWORK' },
    // A dev session unlocks loopback so the suite can reach a fixture. Port 1
    // answers nothing, so this fails at connect. A release build has no dev
    // session and denies loopback outright.
    { label: 'trusted loopback', api: 'downloadFile', args: [{ url: 'http://127.0.0.1:1/a' }], then: 'result', code: 'E_NETWORK' },

    // uploadFile rejects on shape before it opens the file, so these rows need
    // no fixture and no source file -- they hold on every platform.
    { label: 'upload no url', api: 'uploadFile', args: [{}], code: 'E_INVALID_ARG' },
    { label: 'upload no filePath', api: 'uploadFile', args: [{ url: 'http://127.0.0.1:1/upload' }], code: 'E_INVALID_ARG' },
    { label: 'upload empty url', api: 'uploadFile', args: [{ url: '', filePath: 'a.bin' }], code: 'E_INVALID_ARG' },
    { label: 'upload unknown method', api: 'uploadFile', args: [{ url: 'http://127.0.0.1:1/upload', filePath: 'a.bin', method: 'DELETE' }], code: 'E_INVALID_ARG' },
    { label: 'upload unknown bodyMode', api: 'uploadFile', args: [{ url: 'http://127.0.0.1:1/upload', filePath: 'a.bin', bodyMode: 'binary' }], code: 'E_INVALID_ARG' },
    // Multipart-only options under a raw body are rejected, never dropped.
    { label: 'upload raw with formData', api: 'uploadFile', args: [{ url: 'http://127.0.0.1:1/upload', filePath: 'a.bin', bodyMode: 'raw', formData: { note: 'x' } }], code: 'E_INVALID_ARG' },
    { label: 'upload raw with name', api: 'uploadFile', args: [{ url: 'http://127.0.0.1:1/upload', filePath: 'a.bin', bodyMode: 'raw', name: 'asset' }], code: 'E_INVALID_ARG' },
    // Shape is fine here; the file simply is not there.
    { label: 'upload missing file', api: 'uploadFile', args: [{ url: 'http://127.0.0.1:1/upload', filePath: 'no-such-file.bin' }], code: 'E_INVALID_ARG' },
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
    { label: 'stat missing', api: 'fs.stat', args: [missing], code: 'E_NOT_FOUND' },
    { label: 'readDir missing', api: 'fs.readDir', args: [missing], code: 'E_NOT_FOUND' },
    { label: 'file text missing', api: 'fs.file', args: [missing], then: 'text', code: 'E_NOT_FOUND' },
    { label: 'stat empty path', api: 'fs.stat', args: [''], code: 'E_INVALID_ARG' },
    { label: 'stat native absolute', api: 'fs.stat', args: ['/etc/passwd'], code: 'E_INVALID_ARG' },
  ]);

  // exists() answers the same question without throwing at all.
  const exists = await app.logic.eval(async ({ lx }, missing) => [
    await lx.fs.exists(missing),
    await lx.fs.file(missing).exists(),
  ], missing);
  expect(exists).toEqual([false, false]);
});

spec('reject malformed clipboard arguments without touching the OS clipboard', {
  id: 'ARGS-CLIPBOARD-001',
  covers: ['lx.clipboard.write', 'lx.clipboard.writeText', 'lx.clipboard.read'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'ARGS-CLIPBOARD-001');

  await assertRejections(t, app, [
    { label: 'write not object', api: 'clipboard.write', args: ['text'], code: 'E_INVALID_ARG' },
    { label: 'write unknown type', api: 'clipboard.write', args: [{ type: 'html', text: 'x' }], code: 'E_INVALID_ARG' },
    { label: 'write text missing', api: 'clipboard.write', args: [{ type: 'text' }], code: 'E_INVALID_ARG' },
    { label: 'write image empty path', api: 'clipboard.write', args: [{ type: 'image', filePath: '' }], code: 'E_INVALID_ARG' },
    { label: 'write image absolute', api: 'clipboard.write', args: [{ type: 'image', filePath: '/tmp/a.png' }], code: 'E_INVALID_ARG' },
    { label: 'read unknown type', api: 'clipboard.read', args: [{ type: 'html' }], code: 'E_INVALID_ARG' },
    { label: 'read not object', api: 'clipboard.read', args: ['text'], code: 'E_INVALID_ARG' },
    {
      label: 'writeText oversized',
      api: 'clipboard.writeText', args: ['x'.repeat(1024 * 1024 + 1)],
      code: 'E_INVALID_ARG',
    },
  ]);
});
