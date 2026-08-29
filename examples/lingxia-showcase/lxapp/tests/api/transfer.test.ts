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
  covers: ['DownloadTask.abort'],
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

transferSpec('stream monotonic download progress across pause and resume', {
  id: 'TRANSFER-PROGRESS-001',
  covers: ['DownloadTask.next', 'DownloadTask.pause', 'DownloadTask.resume'],
  app: SHOWCASE_APP_ID,
  ...pending,
}, async (t) => {
  const { app, namespace } = bindFixture(t, 'TRANSFER-PROGRESS-001');

  const outcome = await evalCaught(app, `
      const task = lx.downloadFile({
        url: ${JSON.stringify(`${httpBase}/slow.bin?size=1200000&chunks=40&delayMs=60&case=${encodeURIComponent(namespace)}`)},
      });
      const events = [];
      let pauseRequested = false;
      let resumeRequested = false;

      while (true) {
        const step = await task.next();
        if (step.done) break;
        const event = step.value;
        events.push({
          kind: event.kind,
          downloaded: event.downloadedBytes ?? null,
          total: event.totalBytes ?? null,
          progress: event.progress ?? null,
          resultSize: event.result?.size ?? null,
        });

        if (event.kind === 'progress' && !pauseRequested) {
          pauseRequested = true;
          await task.pause();
        } else if (event.kind === 'paused' && !resumeRequested) {
          resumeRequested = true;
          await new Promise((resolve) => setTimeout(resolve, 150));
          await task.resume();
        } else if (event.kind === 'completed') {
          break;
        }
      }

      const waited = await task.wait();
      return { events, waitedSize: waited.size };
  `);
  if (!outcome.ok) {
    throw new Error(`download progress failed: ${JSON.stringify(outcome)}`);
  }
  const result = outcome.value as {
    events: Array<{
      kind: string;
      downloaded: number | null;
      total: number | null;
      progress: number | null;
      resultSize: number | null;
    }>;
    waitedSize: number;
  };

  const kinds = result.events.map((event) => event.kind);
  const pausedAt = kinds.indexOf('paused');
  const resumedAt = kinds.indexOf('resumed');
  expect(pausedAt).toBeGreaterThan(0);
  expect(resumedAt).toBeGreaterThan(pausedAt);
  expect(kinds[kinds.length - 1]).toBe('completed');

  const progress = result.events.filter(
    (event): event is typeof event & { downloaded: number; total: number; progress: number } =>
      event.kind === 'progress'
      && event.downloaded !== null
      && event.total !== null
      && event.progress !== null,
  );
  expect(progress.length).toBeGreaterThan(1);
  expect(progress.every((event, index) =>
    index === 0 || event.downloaded >= progress[index - 1].downloaded)).toBeTruthy();
  expect(progress.every((event) => event.total === 1_200_000)).toBeTruthy();
  expect(progress.every((event) => event.progress >= 0 && event.progress <= 1)).toBeTruthy();

  const completed = result.events[result.events.length - 1];
  expect(completed.resultSize).toBe(1_200_000);
  expect(result.waitedSize).toBe(1_200_000);
});

transferSpec('stop download iteration without canceling the transfer promise', {
  id: 'TRANSFER-RETURN-001',
  covers: ['DownloadTask.next', 'DownloadTask.return', 'DownloadTask.finally'],
  app: SHOWCASE_APP_ID,
  ...pending,
}, async (t) => {
  const { app, namespace } = bindFixture(t, 'TRANSFER-RETURN-001');

  const outcome = await evalCaught(app, `
      const task = lx.downloadFile({
        url: ${JSON.stringify(`${httpBase}/slow.bin?size=131072&chunks=8&delayMs=40&case=${encodeURIComponent(namespace)}`)},
      });
      const first = await task.next();
      const returned = await task.return();
      const afterReturn = await task.next();
      let finallyCount = 0;
      const completed = await task.finally(() => { finallyCount += 1; });
      return {
        firstKind: first.value?.kind ?? null,
        returnedDone: returned.done,
        afterReturnDone: afterReturn.done,
        completedSize: completed.size,
        finallyCount,
      };
  `);
  if (!outcome.ok) {
    throw new Error(`download iterator return failed: ${JSON.stringify(outcome)}`);
  }
  const result = outcome.value as {
    firstKind: string | null;
    returnedDone: boolean;
    afterReturnDone: boolean;
    completedSize: number;
    finallyCount: number;
  };

  expect(result.firstKind).toBe('progress');
  expect(result.returnedDone).toBeTruthy();
  expect(result.afterReturnDone).toBeTruthy();
  expect(result.completedSize).toBe(131_072);
  expect(result.finallyCount).toBe(1);
});

transferSpec('cancel a download through its promise helpers', {
  id: 'TRANSFER-CANCEL-001',
  covers: ['DownloadTask.cancel', 'DownloadTask.catch', 'DownloadTask.finally'],
  app: SHOWCASE_APP_ID,
  ...pending,
}, async (t) => {
  const { app, namespace } = bindFixture(t, 'TRANSFER-CANCEL-001');

  const result = await app.eval({
    timeoutMs: 30_000,
    script: `
      const task = lx.downloadFile({
        url: ${JSON.stringify(`${httpBase}/slow.bin?size=400000&chunks=40&delayMs=100&case=${encodeURIComponent(namespace)}`)},
      });
      let finallyCount = 0;
      const finalized = task.finally(() => { finallyCount += 1; }).catch(() => undefined);
      setTimeout(() => task.cancel(), 300);
      const caught = await task.catch((error) => ({
        code: String(error?.code || ''),
        message: String(error?.message || error),
      }));
      await finalized;
      return { caught, finallyCount };
    `,
  }) as {
    caught: { code: string; message: string };
    finallyCount: number;
  };

  expect(result.caught.code).toBe('E_ABORT');
  expect(result.caught.message).toContain('canceled');
  expect(result.finallyCount).toBe(1);
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

transferSpec('keep the multipart envelope intact whatever the caller heads', {
  id: 'TRANSFER-UPLOAD-002',
  covers: ['lx.uploadFile'],
  app: SHOWCASE_APP_ID,
  ...pending,
}, async (t) => {
  const { app } = bindFixture(t, 'TRANSFER-UPLOAD-002');

  const result = await app.eval({
    timeoutMs: 30_000,
    script: `
      const source = await lx.downloadFile({ url: ${JSON.stringify(`${httpBase}/file/env.bin?size=512`)} });
      // Content-Type carries the boundary the server parses by, so a caller
      // header must not reach it -- unlike bodyMode 'raw', where it must.
      // user-agent is the runtime's to state, and content-length is derived.
      const response = await lx.uploadFile({
        url: ${JSON.stringify(`${httpBase}/upload`)},
        filePath: source.tempFilePath,
        method: 'PUT',
        headers: {
          'Content-Type': 'text/plain',
          'User-Agent': 'spoofed/1.0',
          'Content-Length': '1',
        },
      });
      return JSON.parse(response.data);
    `,
  }) as {
    ok: boolean;
    method: string;
    contentType: string;
    userAgent: string;
    received: number;
    file: { bytes: number } | null;
  };

  expect(result.ok).toBeTruthy();
  // method is orthogonal to body shape: PUT with a multipart envelope is legal.
  expect(result.method).toBe('PUT');
  expect(result.contentType.startsWith('multipart/form-data; boundary=')).toBeTruthy();
  expect(result.userAgent).not.toBe('spoofed/1.0');
  expect(result.file?.bytes).toBe(512);
  // A caller Content-Length of 1 would have truncated the body at the server.
  expect(result.received).toBeGreaterThan(512);
});

transferSpec('stream upload progress that ends on a completed event', {
  id: 'TRANSFER-UPLOAD-PROGRESS-001',
  covers: ['lx.uploadFile', 'UploadTask.next'],
  app: SHOWCASE_APP_ID,
  ...pending,
}, async (t) => {
  const { app } = bindFixture(t, 'TRANSFER-UPLOAD-PROGRESS-001');

  const result = await app.eval({
    timeoutMs: 60_000,
    script: `
      const collect = async (options) => {
        const events = [];
        for await (const event of lx.uploadFile(options)) {
          events.push({ kind: event.kind, uploaded: event.uploadedBytes, total: event.totalBytes });
        }
        return events;
      };
      const source = await lx.downloadFile({ url: ${JSON.stringify(`${httpBase}/file/prog.bin?size=1500000`)} });
      const raw = await collect({
        url: ${JSON.stringify(`${httpBase}/upload-raw`)},
        filePath: source.tempFilePath,
        method: 'PUT',
        bodyMode: 'raw',
      });
      const multipart = await collect({
        url: ${JSON.stringify(`${httpBase}/upload`)},
        filePath: source.tempFilePath,
      });
      return { raw, multipart, size: source.size };
    `,
  }) as {
    raw: { kind: string; uploaded: number; total: number }[];
    multipart: { kind: string; uploaded: number; total: number }[];
    size: number;
  };

  // One shape per mode, so a failure names the property that moved rather than
  // just the assertion that tripped. Progress that goes backwards is worse
  // than no progress at all, and the stream has to end on a terminal event.
  const shape = (events: { kind: string; uploaded: number; total: number }[]) => ({
    streamed: events.length > 1,
    terminal: events[events.length - 1].kind,
    monotonic: events.every((event, i) => i === 0 || event.uploaded >= events[i - 1].uploaded),
    finished: events[events.length - 1].uploaded === events[events.length - 1].total,
  });
  const complete = { streamed: true, terminal: 'completed', monotonic: true, finished: true };
  expect(shape(result.raw)).toEqual(complete);
  expect(shape(result.multipart)).toEqual(complete);

  // Raw carries the file and nothing else; multipart also pays for the
  // envelope, so its total runs above the file size.
  expect(result.raw[0].total).toBe(result.size);
  expect(result.multipart[0].total).toBeGreaterThan(result.size);
});

transferSpec('stop upload iteration and observe rejected promise helpers', {
  id: 'TRANSFER-UPLOAD-HELPERS-001',
  covers: ['UploadTask.return', 'UploadTask.catch', 'UploadTask.finally'],
  app: SHOWCASE_APP_ID,
  ...pending,
}, async (t) => {
  const { app, namespace } = bindFixture(t, 'TRANSFER-UPLOAD-HELPERS-001');

  const result = await app.eval({
    timeoutMs: 60_000,
    script: `
      const source = await lx.downloadFile({
        url: ${JSON.stringify(`${httpBase}/file/helpers.bin?size=1500000&case=${encodeURIComponent(namespace)}`)},
      });

      const completing = lx.uploadFile({
        url: ${JSON.stringify(`${httpBase}/upload?holdMs=500&case=${encodeURIComponent(namespace)}`)},
        filePath: source.tempFilePath,
      });
      const first = await completing.next();
      const returned = await completing.return();
      const afterReturn = await completing.next();
      let successFinallyCount = 0;
      const completed = await completing.finally(() => { successFinallyCount += 1; });

      const rejecting = lx.uploadFile({
        url: ${JSON.stringify(`${httpBase}/upload-raw?reject=403&case=${encodeURIComponent(namespace)}`)},
        filePath: source.tempFilePath,
        method: 'PUT',
        bodyMode: 'raw',
      });
      let rejectFinallyCount = 0;
      const finalized = rejecting
        .finally(() => { rejectFinallyCount += 1; })
        .catch(() => undefined);
      const caught = await rejecting.catch((error) => ({
        code: String(error?.code || ''),
        detail: String(error?.data?.detail || ''),
      }));
      await finalized;

      return {
        firstKind: first.value?.kind ?? null,
        returnedDone: returned.done,
        afterReturnDone: afterReturn.done,
        completedStatus: completed.statusCode,
        successFinallyCount,
        caught,
        rejectFinallyCount,
      };
    `,
  }) as {
    firstKind: string | null;
    returnedDone: boolean;
    afterReturnDone: boolean;
    completedStatus: number;
    successFinallyCount: number;
    caught: { code: string; detail: string };
    rejectFinallyCount: number;
  };

  expect(result.firstKind).toBe('progress');
  expect(result.returnedDone).toBeTruthy();
  expect(result.afterReturnDone).toBeTruthy();
  expect(result.completedStatus).toBe(200);
  expect(result.successFinallyCount).toBe(1);
  expect(result.caught.code).toBe('E_NETWORK');
  expect(result.caught.detail).toContain('403');
  expect(result.rejectFinallyCount).toBe(1);
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

transferSpec('accept PATCH and an empty file as a raw body', {
  id: 'TRANSFER-UPLOAD-RAW-004',
  covers: ['lx.uploadFile'],
  app: SHOWCASE_APP_ID,
  ...pending,
}, async (t) => {
  const { app } = bindFixture(t, 'TRANSFER-UPLOAD-RAW-004');

  const result = await app.eval({
    timeoutMs: 30_000,
    script: `
      // Zero bytes is only reachable with a raw body -- multipart always has an
      // envelope -- and it must still be a well-formed request, not a hang.
      const empty = await lx.downloadFile({ url: ${JSON.stringify(`${httpBase}/file/empty.bin?size=0`)} });
      const response = await lx.uploadFile({
        url: ${JSON.stringify(`${httpBase}/upload-raw`)},
        filePath: empty.tempFilePath,
        method: 'PATCH',
        bodyMode: 'raw',
      });
      return { statusCode: response.statusCode, echo: JSON.parse(response.data) };
    `,
  }) as {
    statusCode: number;
    echo: { method: string; received: number; contentType: string; contentLength: number };
  };

  expect(result.statusCode).toBe(200);
  expect(result.echo.method).toBe('PATCH');
  expect(result.echo.received).toBe(0);
  expect(result.echo.contentLength).toBe(0);
  // No mimeType was given, so the runtime states the neutral default.
  expect(result.echo.contentType).toBe('application/octet-stream');
});

transferSpec('deny an upload to a host the lxapp never trusted', {
  id: 'TRANSFER-UPLOAD-AUTH-001',
  covers: ['lx.uploadFile'],
  app: SHOWCASE_APP_ID,
  ...pending,
}, async (t) => {
  const { app } = bindFixture(t, 'TRANSFER-UPLOAD-AUTH-001');

  // trustedDomains governs uploads exactly as it governs downloads, and the
  // file resolving first must not be mistaken for permission to send it.
  const outcome = await evalCaught(app, `
    const source = await lx.downloadFile({ url: ${JSON.stringify(`${httpBase}/file/auth.bin?size=64`)} });
    return await lx.uploadFile({
      url: 'https://not-trusted.example/upload',
      filePath: source.tempFilePath,
      method: 'PUT',
      bodyMode: 'raw',
    });
  `);

  expect(outcome.ok).toBeFalsy();
  expect(String(outcome.code)).toBe('E_PERMISSION_DENIED');
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
  // A cancel that reports as a connection error is indistinguishable from the
  // network dropping, so E_NETWORK is not an acceptable answer here.
  expect(String(outcome.code)).toBe('E_ABORT');
});
