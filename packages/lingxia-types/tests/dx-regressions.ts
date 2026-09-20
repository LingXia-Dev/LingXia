import type { FsWriteOptions, PageInstance, SystemDownloadsPath, LxFile, DirEntry } from '../src/index.js';
import type { LxAppDriver, PageDriver, PageQueryOptions, DesktopWindowSel, DesktopAppQuitOptions } from '../src/automation/index.js';
import type { RongSpawnSyncOptions } from '../src/process.js';

interface State { count: number }
Page({ data: { count: 0 } as State, onLoad() { this.setData({ count: 1 }); } });
declare const page: PageInstance<{ profile: { name: string } | null; nested?: { count: number }; tuple: [string, number] }>;
page.setPath(['profile', 'name'], 'Alice');
page.setPath(['nested', 'count'], 1);
const first: string = page.data.tuple[0];
// @ts-expect-error tuple positions keep their value types
page.setPath(['tuple', 0], 123);
// @ts-expect-error reserved runtime member
Page({ data: {}, flush() {} });
// @ts-expect-error state cannot contain functions
Page({ data: { action() {} } });
const binaryOptions: FsWriteOptions = { encoding: 'base64' };
// @ts-expect-error named options cannot give binary input a string encoding
lx.fs.write('test.bin', new Uint8Array([1]), binaryOptions);
declare const downloadsPath: SystemDownloadsPath;
// @ts-expect-error system Downloads is outside the managed filesystem
lx.fs.write(downloadsPath, 'x');

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
// @ts-expect-error sync spawning cannot observe asynchronous exit callbacks
const sync: RongSpawnSyncOptions = { onExit() {} };
Rong.$.cwd('/tmp').quiet();
Rong.$.env({ MODE: 'test' }).quiet();

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
  const task = lx.downloadFile({ url: 'https://example.com/file' });
  // @ts-expect-error tasks are not promises
  task.then(() => {});
  // @ts-expect-error iterator methods belong to the progress iterator
  task.next();
  const result = await task.result;
  result.uri.toUpperCase();
  const preview = lx.previewMedia(result.uri);
  const presented = await preview.presented;
  if (presented.status === 'notPresented') presented.reason;
}
void first;
void tracedEval;
void reader;
void taskBoundary;
