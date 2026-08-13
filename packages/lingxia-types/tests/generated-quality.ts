import type {
  AppDownloadFilePath,
  AppDownloadOptions,
  AppDownloadResult,
  AppScreenshotResult,
  AppearanceState,
  DownloadTask,
  DownloadsDownloadOptions,
  DownloadsDownloadResult,
  FileSystemApi,
  HostAppApi,
  Lx,
  LxFile,
  PreviewMediaHandle,
  PreviewMediaOptions,
  SystemDownloadsPath,
  VideoInfo,
} from "../src/index.js";

type Assert<T extends true> = T;
type Not<T extends boolean> = T extends true ? false : true;

declare const lx: Lx;
declare const appDownload: AppDownloadOptions;
declare const downloadsDownload: DownloadsDownloadOptions;
declare const previewOptions: PreviewMediaOptions;
declare const files: FileSystemApi;
declare const app: HostAppApi;
declare const videoInfo: VideoInfo;

const appearanceState: AppearanceState = lx.appearance.get();
const appearanceSetResult: Promise<void> = lx.appearance.set("dark");
const navigationUpdateResult: Promise<void> = lx.navigationBar.update({ title: null });
const tabBarUpdateResult: Promise<void> = lx.tabBar.update({ visibility: "auto" });
const backDefaultResult: Promise<void> = lx.navigateBack();
const backEmptyResult: Promise<void> = lx.navigateBack({});
const backDeltaResult: Promise<void> = lx.navigateBack({ delta: 2 });
// @ts-expect-error delta must be numeric
lx.navigateBack({ delta: "2" });
// @ts-expect-error cross-app navigation accepts configured page names, not routes
lx.navigateToApp({ appId: "lingxia-chat", path: "/pages/chat/index" });
// @ts-expect-error page navigation accepts configured page names, not routes
lx.navigateTo({ path: "/pages/chat/index" });
// @ts-expect-error redirect accepts configured page names, not routes
lx.redirectTo({ path: "/pages/chat/index" });
// @ts-expect-error tab navigation accepts configured page names, not routes
lx.switchTab({ path: "/pages/chat/index" });
// @ts-expect-error relaunch accepts configured page names, not routes
lx.reLaunch({ path: "/pages/chat/index" });
const appDownloadResult: DownloadTask<AppDownloadResult> = lx.downloadFile(appDownload);
const downloadsResult: DownloadTask<DownloadsDownloadResult> = lx.downloadFile(downloadsDownload);
const previewResult: PreviewMediaHandle = lx.previewMedia(previewOptions);
const managedFile: LxFile = files.file("lx://userdata/notes.txt");
const textResult: Promise<string> = managedFile.text();
const bytesResult: Promise<Uint8Array> = managedFile.bytes();
const binaryResult: Promise<ArrayBuffer> = managedFile.arrayBuffer();
const jsonResult: Promise<unknown> = managedFile.json();
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
  typeof appearanceState,
  typeof appearanceSetResult,
  typeof navigationUpdateResult,
  typeof tabBarUpdateResult,
  typeof backDefaultResult,
  typeof backEmptyResult,
  typeof backDeltaResult,
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
