import type {
  AppDownloadFilePath,
  AppPersistedDownloadResult,
  AppTempDownloadResult,
  CompressVideoOptions,
  DownloadProgressEvent,
  Lx,
  UploadOptions,
  UploadProgressEvent,
} from "../src/index.js";

type Assert<T extends true> = T;
type Extends<A, B> = A extends B ? true : false;

declare const lx: Lx;

const rawUpload: UploadOptions = {
  url: "https://example.com/put",
  filePath: "lx://temp/clip.mp4",
  bodyMode: "raw",
  mimeType: "video/mp4",
};

const multipartUpload: UploadOptions = {
  url: "https://example.com/post",
  filePath: "lx://temp/clip.mp4",
  name: "file",
  fileName: "clip.mp4",
  formData: { note: "spec" },
};

void rawUpload;
void multipartUpload;

// @ts-expect-error raw upload cannot carry a multipart field name
const rawWithName: UploadOptions = {
  url: "https://example.com/put",
  filePath: "lx://temp/clip.mp4",
  bodyMode: "raw",
  name: "file",
};

// @ts-expect-error raw upload cannot carry formData
const rawWithForm: UploadOptions = {
  url: "https://example.com/put",
  filePath: "lx://temp/clip.mp4",
  bodyMode: "raw",
  formData: { note: "spec" },
};

// @ts-expect-error raw upload cannot carry fileName
const rawWithFileName: UploadOptions = {
  url: "https://example.com/put",
  filePath: "lx://temp/clip.mp4",
  bodyMode: "raw",
  fileName: "clip.mp4",
};

void rawWithName;
void rawWithForm;
void rawWithFileName;

const preset: CompressVideoOptions = { path: "lx://temp/clip.mp4", quality: "low" };
const manual: CompressVideoOptions = { path: "lx://temp/clip.mp4", bitrate: 800, fps: 24 };
const defaults: CompressVideoOptions = { path: "lx://temp/clip.mp4" };
void preset;
void manual;
void defaults;

// @ts-expect-error quality cannot be combined with bitrate
const mixed: CompressVideoOptions = {
  path: "lx://temp/clip.mp4",
  quality: "low",
  bitrate: 800,
};
void mixed;

async function downloadCorrelation(): Promise<void> {
  const persisted = await lx
    .downloadFile({
      url: "https://example.com/a.bin",
      filePath: "lx://userdata/a.bin",
    })
    .result;
  const filePath: AppDownloadFilePath = persisted.uri;
  void filePath;
  const persistedResult: AppPersistedDownloadResult = persisted;
  void persistedResult;
  type PersistedIsDurable = Assert<typeof persisted extends { storage: 'userdata' } ? true : false>;

  const temporary = await lx.downloadFile({ url: "https://example.com/a.bin" }).result;
  const uri: string = temporary.uri;
  void uri;
  const tempResult: AppTempDownloadResult = temporary;
  void tempResult;
  type TemporaryIsEphemeral = Assert<typeof temporary extends { storage: 'temp' } ? true : false>;
}

async function chooseFileLiteral(): Promise<void> {
  const single = await lx.chooseFile({ multiple: false });
  if (single.status !== 'canceled') {
    const only: string = single.paths[0];
    void only;
    const onlyPath: [string] = single.paths;
    void onlyPath;
  }

  const many = await lx.chooseFile({ multiple: true });
  if (many.status !== 'canceled') {
    const first: string = many.paths[0];
    const rest: string[] = many.paths.slice(1);
    void first;
    void rest;
  }
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
  typeof chooseFileLiteral,
  typeof progressEvents,
  CompletedHasResult,
];
