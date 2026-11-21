import type {
  AppDownloadFilePath,
  AppPersistedDownloadResult,
  AppTempDownloadResult,
  CompressVideoOptions,
  DownloadProgressEvent,
  Lx,
  PickedFile,
  UploadOptions,
  UploadProgressEvent,
  DownloadTask,
  DownloadsDownloadResult,
} from "../src/index.js";

type Assert<T extends true> = T;
type Extends<A, B> = A extends B ? true : false;

declare const lx: Lx;

const rawUpload: UploadOptions = {
  file: "lx://temp/clip.mp4",
  bodyMode: "raw",
  mimeType: "video/mp4",
};

const multipartUpload: UploadOptions = {
  file: "lx://temp/clip.mp4",
  name: "file",
  fileName: "clip.mp4",
  formData: { note: "spec" },
};

void rawUpload;
void multipartUpload;

// @ts-expect-error raw upload cannot carry a multipart field name
const rawWithName: UploadOptions = {
  file: "lx://temp/clip.mp4",
  bodyMode: "raw",
  name: "file",
};

// @ts-expect-error raw upload cannot carry formData
const rawWithForm: UploadOptions = {
  file: "lx://temp/clip.mp4",
  bodyMode: "raw",
  formData: { note: "spec" },
};

// @ts-expect-error raw upload cannot carry fileName
const rawWithFileName: UploadOptions = {
  file: "lx://temp/clip.mp4",
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
  const filePath: AppDownloadFilePath = persisted.uri;
  void filePath;
  const persistedResult: AppPersistedDownloadResult = persisted;
  void persistedResult;
  type PersistedIsDurable = Assert<typeof persisted extends { storage: 'userdata' } ? true : false>;

  const temporary = await lx.network.download("https://example.com/a.bin").result;
  const uri: string = temporary.uri;
  void uri;
  const tempResult: AppTempDownloadResult = temporary;
  void tempResult;
  type TemporaryIsEphemeral = Assert<typeof temporary extends { storage: 'temp' } ? true : false>;
}

function transferShapes(): void {
  lx.network.upload("https://example.com/put", { file: "lx://temp/clip.mp4" });
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
    const only: [PickedFile] = single.files;
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

function progressEvents(
  download: DownloadProgressEvent,
  upload: UploadProgressEvent,
): void {
  if (download.kind === "completed") {
    const size: number = download.result.sizeBytes;
    void size;
  } else {
    // @ts-expect-error result is only on completed
    const result = download.result;
    void result;
  }

  if (upload.kind === "completed") {
    const status: number = upload.result.statusCode;
    void status;
  } else {
    // @ts-expect-error result is only on completed
    const result = upload.result;
    void result;
  }
}

type CompletedHasResult = Assert<
  Extends<Extract<DownloadProgressEvent, { kind: "completed" }>, { result: unknown }>
>;

export type ExclusiveOptionsGate = [
  typeof downloadCorrelation,
  typeof pickFileShapes,
  typeof progressEvents,
  CompletedHasResult,
];
