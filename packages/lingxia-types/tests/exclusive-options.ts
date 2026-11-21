import type {
  AppDownloadFilePath,
  AppPersistedDownloadResult,
  AppTempDownloadResult,
  CompressVideoOptions,
  Lx,
  PickedFile,
  UploadOptions,
  TransferProgress,
  UploadResult,
  CompressVideoResult,
  HostAppUpdateTask,
  HostAppUpdateResult,
  DownloadTask,
  DownloadsDownloadResult,
} from "../src/index.js";

type Assert<T extends true> = T;
type Extends<A, B> = A extends B ? true : false;

declare const lx: Lx;

const rawUpload: UploadOptions = {
  path: "lx://temp/clip.mp4",
  bodyMode: "raw",
  type: "video/mp4",
};

const multipartUpload: UploadOptions = {
  path: "lx://temp/clip.mp4",
  field: "file",
  fileName: "clip.mp4",
  fields: { note: "spec" },
};

void rawUpload;
void multipartUpload;

// @ts-expect-error raw upload cannot carry a multipart field name
const rawWithName: UploadOptions = {
  path: "lx://temp/clip.mp4",
  bodyMode: "raw",
  field: "file",
};

// @ts-expect-error raw upload cannot carry fields
const rawWithForm: UploadOptions = {
  path: "lx://temp/clip.mp4",
  bodyMode: "raw",
  fields: { note: "spec" },
};

// @ts-expect-error raw upload cannot carry fileName
const rawWithFileName: UploadOptions = {
  path: "lx://temp/clip.mp4",
  bodyMode: "raw",
  fileName: "clip.mp4",
};

void rawWithName;
void rawWithForm;
void rawWithFileName;

const preset: CompressVideoOptions = { quality: "low" };
const manual: CompressVideoOptions = { bitrate: 800, fps: 24 };
const defaults: CompressVideoOptions = { to: "lx://userdata/clip.mp4" };
void preset;
void manual;
void defaults;

// @ts-expect-error quality cannot be combined with bitrate
const mixed: CompressVideoOptions = {
  quality: "low",
  bitrate: 800,
};
void mixed;

async function downloadCorrelation(): Promise<void> {
  const persisted = await lx
    .network.download("https://example.com/a.bin", {
      to: "lx://userdata/a.bin",
    })
    .result;
  const filePath: AppDownloadFilePath = persisted.path;
  void filePath;
  const persistedResult: AppPersistedDownloadResult = persisted;
  void persistedResult;
  type PersistedIsDurable = Assert<typeof persisted extends { storage: 'userdata' } ? true : false>;

  const temporary = await lx.network.download("https://example.com/a.bin").result;
  const path: string = temporary.path;
  void path;
  const tempResult: AppTempDownloadResult = temporary;
  void tempResult;
  type TemporaryIsEphemeral = Assert<typeof temporary extends { storage: 'temp' } ? true : false>;
}

function transferShapes(): void {
  lx.network.upload("https://example.com/put", { path: "lx://temp/clip.mp4" });
  const downloads: DownloadTask<DownloadsDownloadResult> = lx.network.download("https://example.com/a.bin", {
    to: "downloads",
    suggestedName: "a.bin",
  });
  void downloads;
  // @ts-expect-error the URL is the positional subject
  lx.network.download({ url: "https://example.com/a.bin" });
  // @ts-expect-error suggestedName names a file in Downloads only
  lx.network.download("https://example.com/a.bin", { to: "lx://userdata/a.bin", suggestedName: "a.bin" });
  // @ts-expect-error upload needs the file
  lx.network.upload("https://example.com/put", {});
}

async function pickFileShapes(): Promise<void> {
  const single = await lx.pick.file();
  if (single.status !== 'canceled') {
    const only: readonly [PickedFile] = single.files;
    // @ts-expect-error pick.file returns exactly one file
    single.files[1].path;
    void only;
  }

  const many = await lx.pick.files();
  if (many.status !== 'canceled') {
    const first: PickedFile = many.files[0];
    const rest: PickedFile[] = many.files.slice(1);
    void first;
    void rest;
  }

  // @ts-expect-error `multiple` is gone: pick.file is one, pick.files is many
  await lx.pick.file({ multiple: true });
}

// Every task is its result plus one stream of `TransferProgress` that returns the result.
async function taskShape(task: DownloadTask<AppTempDownloadResult>): Promise<void> {
  for await (const progress of task) {
    const loaded: number = progress.loaded;
    const fraction: number | undefined = progress.fraction;
    void loaded;
    void fraction;
    // @ts-expect-error progress carries no event kind
    void progress.kind;
  }
  // @ts-expect-error progress is the task itself
  void task.progress;
  const step = await task[Symbol.asyncIterator]().next();
  if (step.done) {
    const path: string = step.value.path;
    void path;
  }
}

async function* streamDownload(url: string): AsyncGenerator<TransferProgress, AppTempDownloadResult> {
  return yield* lx.network.download(url);
}

async function uploadAnswers(options: UploadOptions): Promise<void> {
  const answer: UploadResult = await lx.network.upload("https://example.com/upload", options).result;
  if (!answer.ok) {
    const status: number = answer.status;
    const body: string = answer.text;
    void status;
    void body;
  }
  const parsed: { id: string } = answer.json<{ id: string }>();
  void parsed;
  // @ts-expect-error the status is `status`
  void answer.statusCode;
}

async function otherTasks(clip: string, update: HostAppUpdateTask): Promise<void> {
  const compress = lx.media.compressVideo(clip);
  for await (const { fraction } of compress) void fraction;
  const compressed: CompressVideoResult = await compress.result;
  void compressed;
  for await (const { loaded } of update) void loaded;
  const applied: HostAppUpdateResult = await update.result;
  void applied;
  // @ts-expect-error an update cannot be paused
  void update.pause;
}

export type ExclusiveOptionsGate = [
  typeof downloadCorrelation,
  typeof pickFileShapes,
  typeof taskShape,
  typeof streamDownload,
  typeof uploadAnswers,
  typeof otherTasks,
];
