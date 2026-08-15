import type {
  AppDownloadFilePath,
  AppDownloadOptions,
  AppDownloadResult,
  AppScreenshotResult,
  AppearanceState,
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

const appearanceState: AppearanceState = lx.appearance.get();
const appearanceSetResult: Promise<void> = lx.appearance.set("dark");
const navigationUpdateResult: Promise<void> = lx.navigationBar.update({ title: null });
const tabBarUpdateResult: Promise<void> = lx.tabBar.update({ visibility: "auto" });
const backDefaultResult: Promise<void> = lx.navigateBack();
const backEmptyResult: Promise<void> = lx.navigateBack({});
const backDeltaResult: Promise<void> = lx.navigateBack({ delta: 2 });
// @ts-expect-error delta must be numeric
lx.navigateBack({ delta: "2" });
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
// @ts-expect-error dynamic app surfaces support shell main/aside roles; floats use declarations
lx.openSurface({ appId: "lingxia-chat", as: "float" });
// @ts-expect-error dynamic app surfaces accept configured page names, not routes
lx.openSurface({ appId: "lingxia-chat", as: "main", path: "/pages/chat/index" });
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
// @ts-expect-error provider implementation kinds are not public selectors
lx.openSurface({ native: "terminal" });
// The declaration may supply the aside role when `as` is omitted.
lx.openSurface({ surface: "terminal", edge: "right" });
lx.openSurface({ surface: "terminal", as: "aside", edge: "left" });
// @ts-expect-error an explicit main declaration cannot accept an aside edge
lx.openSurface({ surface: "terminal", as: "main", edge: "right" });
// @ts-expect-error dynamic main app surfaces do not accept an aside edge
lx.openSurface({ appId: "lingxia-chat", as: "main", edge: "right" });
// @ts-expect-error a spec must not mix selectors
lx.openSurface({ page: "settings", as: "float", surface: "terminal" });
// @ts-expect-error key belongs only to a declared surface
lx.openSurface({ appId: "lingxia-chat", as: "main", key: "project-a" });
// @ts-expect-error interaction belongs only to page surfaces
lx.openSurface({ surface: "terminal", interaction: { closeButton: true } });
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
  typeof appearanceState,
  typeof appearanceSetResult,
  typeof navigationUpdateResult,
  typeof tabBarUpdateResult,
  typeof backDefaultResult,
  typeof backEmptyResult,
  typeof backDeltaResult,
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
