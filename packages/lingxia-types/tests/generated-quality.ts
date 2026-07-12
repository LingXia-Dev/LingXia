import type {
  AppDownloadFilePath,
  AppDownloadOptions,
  AppDownloadResult,
  AppScreenshotResult,
  DownloadTask,
  DownloadsDownloadOptions,
  DownloadsDownloadResult,
  FileManager,
  HostAppApi,
  Lx,
  OpenDeclaredSurfaceSpec,
  OpenPageSurfaceSpec,
  OpenUrlAsideSpec,
  OpenUrlTabSpec,
  PreviewMediaHandle,
  PreviewMediaOptions,
  ReadBinaryFileOptions,
  ReadBinaryFileResult,
  ReadTextFileOptions,
  ReadTextFileResult,
  Surface,
  SurfaceHandle,
  SystemDownloadsPath,
} from "../src/index.js";

type Assert<T extends true> = T;
type Not<T extends boolean> = T extends true ? false : true;

declare const lx: Lx;
declare const urlTab: OpenUrlTabSpec;
declare const declaredSurface: OpenDeclaredSurfaceSpec;
declare const pageSurface: OpenPageSurfaceSpec;
declare const urlAside: OpenUrlAsideSpec;
declare const appDownload: AppDownloadOptions;
declare const downloadsDownload: DownloadsDownloadOptions;
declare const previewOptions: PreviewMediaOptions;
declare const files: FileManager;
declare const readText: ReadTextFileOptions;
declare const readBinary: ReadBinaryFileOptions;
declare const app: HostAppApi;

const urlTabResult: Promise<null> = lx.openSurface(urlTab);
const declaredResult: Promise<SurfaceHandle> = lx.openSurface(declaredSurface);
const pageResult: Promise<Surface> = lx.openSurface(pageSurface);
const asideResult: Promise<Surface> = lx.openSurface(urlAside);
const appDownloadResult: DownloadTask<AppDownloadResult> = lx.downloadFile(appDownload);
const downloadsResult: DownloadTask<DownloadsDownloadResult> = lx.downloadFile(downloadsDownload);
const previewResult: PreviewMediaHandle = lx.previewMedia(previewOptions);
const textResult: Promise<ReadTextFileResult> = files.readFile(readText);
const binaryResult: Promise<ReadBinaryFileResult> = files.readFile(readBinary);
const screenshotResult: Promise<AppScreenshotResult> = app.screenshot();

type AppPathIsBranded = Assert<Not<string extends AppDownloadFilePath ? true : false>>;
type DownloadsPathIsBranded = Assert<Not<string extends SystemDownloadsPath ? true : false>>;
type BrandsStayDistinct = Assert<Not<AppDownloadFilePath extends SystemDownloadsPath ? true : false>>;

export type GeneratedQualityGate = [
  typeof urlTabResult,
  typeof declaredResult,
  typeof pageResult,
  typeof asideResult,
  typeof appDownloadResult,
  typeof downloadsResult,
  typeof previewResult,
  typeof textResult,
  typeof binaryResult,
  typeof screenshotResult,
  AppPathIsBranded,
  DownloadsPathIsBranded,
  BrandsStayDistinct,
];
