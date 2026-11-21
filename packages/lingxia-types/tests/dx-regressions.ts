import type { FsWriteOptions, SystemDownloadsPath, LxFile, DirEntry } from '../src/index.js';
import type { LxAppDriver, PageDriver, PageQueryOptions, DesktopWindowSel, DesktopAppQuitOptions } from '../src/automation/index.js';

// What `import { lx } from '@lingxia/logic'` gives.
declare const lx: import('../src/index.js').Lx;

const binaryOptions: Pick<FsWriteOptions, 'encoding'> = { encoding: 'base64' };
// @ts-expect-error named options cannot give binary input a string encoding
lx.fs.write('test.bin', { data: new Uint8Array([1]), ...binaryOptions });
declare const downloadsPath: SystemDownloadsPath;
// @ts-expect-error system Downloads is outside the managed filesystem
lx.fs.write(downloadsPath, { data: 'x' });

declare const driver: PageDriver;
declare const options: PageQueryOptions;
driver.query(options);
declare const app: LxAppDriver;
async function tracedEval() {
  const traced = await app.eval<number>({ script: '1', captureCalls: true });
  traced.value.toFixed();
  // @ts-expect-error tracing wraps the value
  traced.toFixed();
}
// @ts-expect-error exactly one window selector is required
const noTarget: DesktopWindowSel = {};
// @ts-expect-error cannot select multiple quit targets
const twoTargets: DesktopAppQuitOptions = { pid: 1, match: 'app' };

new ReadableStream<Uint8Array>({ start(controller) {
  controller.enqueue(new Uint8Array([1]));
  // @ts-expect-error controller is typed to the stream's element
  controller.enqueue('wrong');
} });
async function reader() {
  const result = await new ReadableStream<Uint8Array>().getReader().read();
  // @ts-expect-error a finished reader does not return a chunk
  result.value.byteLength;
  if (!result.done) result.value.byteLength;
}
// @ts-expect-error native handles are types, not JavaScript exports
void LxFile;
// @ts-expect-error native entries are types, not JavaScript exports
void DirEntry;

async function taskBoundary() {
  const task = lx.network.download('https://example.com/file');
  // @ts-expect-error tasks are not promises
  task.then(() => {});
  // @ts-expect-error iterator methods belong to the progress iterator
  task.next();
  const result = await task.result;
  result.uri.toUpperCase();
  const preview = lx.media.preview(result.uri);
  const presented = await preview.presented;
  if (presented.status === 'notPresented') presented.reason;
}
void tracedEval;
void reader;
void taskBoundary;
