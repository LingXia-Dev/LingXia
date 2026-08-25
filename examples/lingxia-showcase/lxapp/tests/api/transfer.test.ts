import { expect, spec } from '@lingxia/test';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { bindFixture, evalCaught } from '../helpers/poll.js';

/**
 * The fixture server is started outside the runtime (see tests/harness). With
 * no `--arg httpBase=…` these register as pending with the reason, rather than
 * vanishing from the report as if transfer were covered.
 */
const httpBase = (globalThis.__LINGXIA_AUTOMATION_HOST__?.args ?? {}).httpBase;
const transferSpec = httpBase ? spec : spec.skip;
const pending = {
  reason: 'needs the HTTP fixture: node tests/harness/http-fixture.mjs, then --arg httpBase=<url>',
};

transferSpec('download a body and read the bytes back out of the sandbox', {
  id: 'TRANSFER-DOWNLOAD-001',
  covers: ['lx.downloadFile', 'DownloadTask.then', 'DownloadTask.wait'],
  app: SHOWCASE_APP_ID,
  ...pending,
}, async (t) => {
  const { app } = bindFixture(t, 'TRANSFER-DOWNLOAD-001');

  const result = await app.eval({
    timeoutMs: 30_000,
    script: `
      const awaited = await lx.downloadFile({ url: ${JSON.stringify(`${httpBase}/file/a.bin?size=4096`)} });
      // wait() and await must describe the same completed transfer.
      const waited = await lx.downloadFile({ url: ${JSON.stringify(`${httpBase}/file/b.bin?size=4096`)} }).wait();
      const stat = await lx.fs.stat(awaited.tempFilePath);
      return {
        awaitedSize: awaited.size,
        waitedSize: waited.size,
        onDisk: stat.size,
        managed: awaited.tempFilePath.startsWith('lx://'),
      };
    `,
  }) as { awaitedSize: number; waitedSize: number; onDisk: number; managed: boolean };

  expect(result.awaitedSize).toBe(4096);
  expect(result.waitedSize).toBe(4096);
  // A download the lxapp cannot read back is not a download.
  expect(result.onDisk).toBe(4096);
  expect(result.managed).toBeTruthy();
});

transferSpec('abort an in-flight download and reject with E_ABORT', {
  id: 'TRANSFER-ABORT-001',
  covers: ['DownloadTask.abort', 'DownloadTask.catch'],
  app: SHOWCASE_APP_ID,
  ...pending,
}, async (t) => {
  const { app } = bindFixture(t, 'TRANSFER-ABORT-001');

  const outcome = await evalCaught(app, `
    const task = lx.downloadFile({
      url: ${JSON.stringify(`${httpBase}/slow?size=400000&chunks=40&delayMs=100`)},
    });
    setTimeout(() => task.abort(), 300);
    return await task;
  `);

  expect(outcome.ok).toBeFalsy();
  expect(outcome.code).toBe('E_ABORT');
});

transferSpec('report the server status a failed download saw', {
  id: 'TRANSFER-STATUS-001',
  covers: ['lx.downloadFile'],
  app: SHOWCASE_APP_ID,
  ...pending,
}, async (t) => {
  const { app } = bindFixture(t, 'TRANSFER-STATUS-001');

  for (const status of [404, 500, 503]) {
    await t.step(`http ${status}`, async () => {
      const outcome = await evalCaught(app, `
        return await lx.downloadFile({
          url: ${JSON.stringify(`${httpBase}/status?code=`)} + ${status},
        });
      `);
      expect(outcome.ok).toBeFalsy();
      expect(outcome.code).toBe('E_NETWORK');
      // The status has to reach the caller, or a 404 is indistinguishable
      // from a dropped connection.
      expect(String((outcome.data as { detail?: string } | undefined)?.detail)).toContain(String(status));
    });
  }
});

transferSpec('upload a managed file as multipart and read the server echo', {
  id: 'TRANSFER-UPLOAD-001',
  covers: ['lx.uploadFile', 'UploadTask.then', 'UploadTask.wait'],
  app: SHOWCASE_APP_ID,
  ...pending,
}, async (t) => {
  const { app } = bindFixture(t, 'TRANSFER-UPLOAD-001');

  const result = await app.eval({
    timeoutMs: 30_000,
    script: `
      const source = await lx.downloadFile({ url: ${JSON.stringify(`${httpBase}/file/up.bin?size=1024`)} });
      const response = await lx.uploadFile({
        url: ${JSON.stringify(`${httpBase}/upload`)},
        filePath: source.tempFilePath,
        name: 'asset',
        fileName: 'up.bin',
        formData: { note: 'spec' },
        headers: { 'x-lx-test': 'echo' },
      });
      return { statusCode: response.statusCode, echo: JSON.parse(response.data) };
    `,
  }) as {
    statusCode: number;
    echo: {
      ok: boolean;
      file: { field: string; filename: string; bytes: number };
      fields: Record<string, string>;
      headerEcho: string;
    };
  };

  expect(result.statusCode).toBe(200);
  expect(result.echo.ok).toBeTruthy();
  // Every option the caller passed has to survive the multipart envelope.
  expect(result.echo.file.field).toBe('asset');
  expect(result.echo.file.filename).toBe('up.bin');
  expect(result.echo.file.bytes).toBe(1024);
  expect(result.echo.fields).toEqual({ note: 'spec' });
  expect(result.echo.headerEcho).toBe('echo');
});

transferSpec('upload a raw body with PUT for presigned endpoints', {
  id: 'TRANSFER-UPLOAD-RAW-001',
  covers: ['lx.uploadFile'],
  app: SHOWCASE_APP_ID,
  ...pending,
}, async (t) => {
  const { app } = bindFixture(t, 'TRANSFER-UPLOAD-RAW-001');

  const result = await app.eval({
    timeoutMs: 30_000,
    script: `
      const source = await lx.downloadFile({ url: ${JSON.stringify(`${httpBase}/file/raw.bin?size=2048`)} });
      const response = await lx.uploadFile({
        url: ${JSON.stringify(`${httpBase}/upload-raw`)},
        filePath: source.tempFilePath,
        method: 'PUT',
        bodyMode: 'raw',
        mimeType: 'application/x-lingxia-test',
        headers: { 'x-lx-test': 'echo' },
      });
      // A presigned signature covers Content-Type, so an explicit header has
      // to win over mimeType rather than be rewritten by the runtime.
      const overridden = await lx.uploadFile({
        url: ${JSON.stringify(`${httpBase}/upload-raw`)},
        filePath: source.tempFilePath,
        method: 'PUT',
        bodyMode: 'raw',
        mimeType: 'application/x-lingxia-test',
        headers: { 'Content-Type': 'image/avif' },
      });
      return {
        statusCode: response.statusCode,
        echo: JSON.parse(response.data),
        overriddenContentType: JSON.parse(overridden.data).contentType,
      };
    `,
  }) as {
    statusCode: number;
    echo: {
      ok: boolean;
      method: string;
      received: number;
      contentType: string;
      contentLength: number;
      firstBytes: string;
      headerEcho: string;
    };
    overriddenContentType: string;
  };

  expect(result.statusCode).toBe(200);
  expect(result.echo.method).toBe('PUT');
  // Raw means the file bytes and nothing else: no multipart framing overhead,
  // and a Content-Length taken from the file, which presigned PUT endpoints
  // require. The fixture fills byte i with i % 251.
  expect(result.echo.received).toBe(2048);
  expect(result.echo.contentLength).toBe(2048);
  expect(result.echo.firstBytes).toBe('0001020304050607');
  expect(result.echo.contentType).toBe('application/x-lingxia-test');
  expect(result.echo.headerEcho).toBe('echo');
  expect(result.overriddenContentType).toBe('image/avif');
});

transferSpec('reject multipart-only options when the body is raw', {
  id: 'TRANSFER-UPLOAD-RAW-002',
  covers: ['lx.uploadFile'],
  app: SHOWCASE_APP_ID,
  ...pending,
}, async (t) => {
  const { app } = bindFixture(t, 'TRANSFER-UPLOAD-RAW-002');

  // Dropping these silently would leave the lxapp believing they were sent.
  const rejectedFormData = await evalCaught(app, `
    const source = await lx.downloadFile({ url: ${JSON.stringify(`${httpBase}/file/raw.bin?size=64`)} });
    return await lx.uploadFile({
      url: ${JSON.stringify(`${httpBase}/upload-raw`)},
      filePath: source.tempFilePath,
      method: 'PUT',
      bodyMode: 'raw',
      formData: { note: 'spec' },
    });
  `);
  const rejectedName = await evalCaught(app, `
    const source = await lx.downloadFile({ url: ${JSON.stringify(`${httpBase}/file/raw.bin?size=64`)} });
    return await lx.uploadFile({
      url: ${JSON.stringify(`${httpBase}/upload-raw`)},
      filePath: source.tempFilePath,
      method: 'PUT',
      bodyMode: 'raw',
      name: 'asset',
    });
  `);

  expect(rejectedFormData.ok).toBeFalsy();
  expect(String(rejectedFormData.code)).toBe('E_INVALID_ARG');
  expect(rejectedName.ok).toBeFalsy();
  expect(String(rejectedName.code)).toBe('E_INVALID_ARG');
});

transferSpec('report the refusing status when a raw upload is rejected mid-body', {
  id: 'TRANSFER-UPLOAD-RAW-003',
  covers: ['lx.uploadFile'],
  app: SHOWCASE_APP_ID,
  ...pending,
}, async (t) => {
  const { app } = bindFixture(t, 'TRANSFER-UPLOAD-RAW-003');

  // How a presigned URL refuses a signature: answer, then hang up before the
  // body is done. The status has to survive that, or the lxapp cannot tell a
  // rejected signature from a flaky network.
  const outcome = await evalCaught(app, `
    const source = await lx.downloadFile({ url: ${JSON.stringify(`${httpBase}/file/reject.bin?size=8000000`)} });
    return await lx.uploadFile({
      url: ${JSON.stringify(`${httpBase}/upload-raw?reject=403`)},
      filePath: source.tempFilePath,
      method: 'PUT',
      bodyMode: 'raw',
    });
  `);

  expect(outcome.ok).toBeFalsy();
  expect(String((outcome.data as { detail?: string } | undefined)?.detail)).toContain('403');
});

transferSpec('cancel an upload and reject rather than resolve', {
  id: 'TRANSFER-UPLOAD-CANCEL-001',
  covers: ['UploadTask.cancel'],
  app: SHOWCASE_APP_ID,
  ...pending,
}, async (t) => {
  const { app } = bindFixture(t, 'TRANSFER-UPLOAD-CANCEL-001');

  const outcome = await evalCaught(app, `
    const source = await lx.downloadFile({ url: ${JSON.stringify(`${httpBase}/file/big.bin?size=2000000`)} });
    // holdMs keeps the request open long enough for a cancel to be meaningful.
    const task = lx.uploadFile({
      url: ${JSON.stringify(`${httpBase}/upload?holdMs=3000`)},
      filePath: source.tempFilePath,
    });
    setTimeout(() => task.cancel(), 200);
    return await task;
  `);

  expect(outcome.ok).toBeFalsy();
  expect(String(outcome.code)).toMatch(/E_ABORT|E_NETWORK/);
});
