import type {
  AppDownloadFilePath,
  AppDownloadOptions,
  AppDownloadResult,
  AppScreenshotResult,
  CapsuleRect,
  DownloadTask,
  DownloadsDownloadOptions,
  DownloadsDownloadResult,
  FileManager,
  HostAppApi,
  Lx,
  OpenDeclaredSurfaceSpec,
  OpenAppSurfaceSpec,
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
  VideoInfo,
} from "../src/index.js";

type Assert<T extends true> = T;
type Not<T extends boolean> = T extends true ? false : true;

declare const lx: Lx;
declare const urlTab: OpenUrlTabSpec;
declare const declaredSurface: OpenDeclaredSurfaceSpec;
declare const appSurface: OpenAppSurfaceSpec;
declare const pageSurface: OpenPageSurfaceSpec;
declare const urlAside: OpenUrlAsideSpec;
declare const appDownload: AppDownloadOptions;
declare const downloadsDownload: DownloadsDownloadOptions;
declare const previewOptions: PreviewMediaOptions;
declare const files: FileManager;
declare const readText: ReadTextFileOptions;
declare const readBinary: ReadBinaryFileOptions;
declare const app: HostAppApi;
declare const videoInfo: VideoInfo;

const capsuleRectResult: Promise<CapsuleRect | null> = lx.getCapsuleRect();
const urlTabResult: Promise<null> = lx.openSurface(urlTab);
const declaredResult: Promise<SurfaceHandle> = lx.openSurface(declaredSurface);
const appResult: Promise<SurfaceHandle> = lx.openSurface(appSurface);
const terminalWorkspaceResult: Promise<SurfaceHandle> = lx.openSurface({
  surface: "terminal",
  key: "project-a",
  as: "aside",
});
const dynamicAppResult: Promise<SurfaceHandle> = lx.openSurface({
  appId: "lingxia-chat",
  as: "main",
  page: "chat",
});
// @ts-expect-error app Surface composition requires an explicit role
lx.openSurface({ appId: "lingxia-chat" });
// @ts-expect-error provider implementation kinds are not public selectors
lx.openSurface({ native: "terminal" });
const pageResult: Promise<Surface> = lx.openSurface(pageSurface);
const asideResult: Promise<Surface | null> = lx.openSurface(urlAside);
const appDownloadResult: DownloadTask<AppDownloadResult> = lx.downloadFile(appDownload);
const downloadsResult: DownloadTask<DownloadsDownloadResult> = lx.downloadFile(downloadsDownload);
const previewResult: PreviewMediaHandle = lx.previewMedia(previewOptions);
const textResult: Promise<ReadTextFileResult> = files.readFile(readText);
const binaryResult: Promise<ReadBinaryFileResult> = files.readFile(readBinary);
const screenshotResult: Promise<AppScreenshotResult> = app.screenshot();
const videoSize: number = videoInfo.size;
const videoPath: string = videoInfo.path;
const videoCodec: string | undefined = videoInfo.videoCodec;
const hasAudio: boolean | undefined = videoInfo.hasAudio;
const audioCodec: string | undefined = videoInfo.audioCodec;

type AppPathIsBranded = Assert<Not<string extends AppDownloadFilePath ? true : false>>;
type DownloadsPathIsBranded = Assert<Not<string extends SystemDownloadsPath ? true : false>>;
type BrandsStayDistinct = Assert<Not<AppDownloadFilePath extends SystemDownloadsPath ? true : false>>;

export type GeneratedQualityGate = [
  typeof capsuleRectResult,
  typeof urlTabResult,
  typeof declaredResult,
  typeof appResult,
  typeof dynamicAppResult,
  typeof terminalWorkspaceResult,
  typeof pageResult,
  typeof asideResult,
  typeof appDownloadResult,
  typeof downloadsResult,
  typeof previewResult,
  typeof textResult,
  typeof binaryResult,
  typeof screenshotResult,
  typeof videoSize,
  typeof videoPath,
  typeof videoCodec,
  typeof hasAudio,
  typeof audioCodec,
  AppPathIsBranded,
  DownloadsPathIsBranded,
  BrandsStayDistinct,
];
