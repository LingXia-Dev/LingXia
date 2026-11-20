import type {
  PageHandle,
  AppDownloadFilePath,
  AppDownloadOptions,
  AppDownloadResult,
  AppLaunchOptions,
  AppLaunchScene,
  AppScreenshotResult,
  AppearancePreference,
  ResolvedAppearance,
  DownloadTask,
  DownloadsDownloadOptions,
  DownloadsDownloadResult,
  ClipboardTextResult,
  ClipboardType,
  FileSystemApi,
  HostAppApi,
  HostOs,
  Lx,
  LxFile,
  PreviewMediaHandle,
  PreviewMediaOptions,
  SystemDownloadsPath,
  VideoInfo,
} from "../src/index.js";
import type { PageApi } from "../src/page.js";

type Assert<T extends true> = T;
type Not<T extends boolean> = T extends true ? false : true;

declare const lx: Lx;
declare const page: PageApi<{}, {}, never>;
declare const appDownload: AppDownloadOptions;
declare const downloadsDownload: DownloadsDownloadOptions;
declare const previewOptions: PreviewMediaOptions;
declare const files: FileSystemApi;
declare const app: HostAppApi;
declare const videoInfo: VideoInfo;

const resolvedAppearance: ResolvedAppearance = lx.host.appearance.get();
const appearanceUnwatch: () => void = lx.host.appearance.watch(() => {});
const appearanceSetResult: Promise<void> | undefined =
  lx.host.appearance.setPreference("dark");
const appearancePreference: AppearancePreference | undefined =
  lx.host.appearance.getPreference();
const navigationUpdateResult: Promise<void> = page.navigationBar.update({ title: null });
const tabBarUpdateResult: Promise<void> = lx.app.tabBar.update({ visibility: "auto" });
const forcedTabBarUpdateResult: Promise<void> = lx.app.tabBar.update({ visibility: "visible" });
// @ts-expect-error visible is TabBar-only; homeButton remains auto/hidden
page.navigationBar.update({ homeButton: "visible" });
const backDefaultResult: Promise<void> = lx.nav.back();
const backCountResult: Promise<void> = lx.nav.back(2);
// @ts-expect-error back takes a page count
lx.nav.back({ delta: 2 });
const pushed: PageHandle = lx.nav.go({ page: "detail", params: { id: 1 } });
const replaced: Promise<void> = lx.nav.go({ page: "detail" }, { replace: true });
const tabbed: Promise<void> = lx.nav.go({ tab: "home" });
const otherApp: Promise<void> = lx.nav.go({ app: "lingxia-chat", params: { room: "a" } });
// @ts-expect-error a target names one destination
lx.nav.go({ page: "detail", tab: "home" });
// @ts-expect-error a tab takes no params
lx.nav.go({ tab: "home", params: {} });
// @ts-expect-error only a page replaces the current one
lx.nav.go({ tab: "home" }, { replace: true });
// @ts-expect-error reset starts at a page or a tab
lx.nav.reset({ url: "https://example.com" });
// @ts-expect-error navigation takes configured page names, not routes
lx.nav.go({ path: "/pages/chat/index" });
// @ts-expect-error a target takes params, not query
lx.nav.go({ page: "detail", query: {} });
// @ts-expect-error going back is lx.nav.back(n)
lx.nav.go(-1);
const appDownloadResult: DownloadTask<AppDownloadResult> = lx.network.download(appDownload);
const downloadsResult: DownloadTask<DownloadsDownloadResult> = lx.network.download(downloadsDownload);
const previewResult: PreviewMediaHandle = lx.media.preview(previewOptions);
const managedFile: LxFile = files.file("lx://userdata/notes.txt");
const textResult: Promise<string> = managedFile.text();
const bytesResult: Promise<Uint8Array> = managedFile.bytes();
const binaryResult: Promise<ArrayBuffer> = managedFile.arrayBuffer();
const jsonResult: Promise<unknown> = managedFile.json();
const screenshotResult: Promise<AppScreenshotResult> = app.screenshot();
const clipboardWrite: Promise<void> = lx.clipboard.writeText("copied");
const clipboardText: Promise<ClipboardTextResult> = lx.clipboard.readText();
const clipboardTypes: Promise<ClipboardType[]> = lx.clipboard.types();
// Control-only is a run-time refusal, not an absent member (rule 2).
const cacheSize: Promise<number> = lx.host.cache.size();
const notifyPermission = lx.host.notification.getPermission();
const hostOs: HostOs = lx.host.info.os;
const deviceOs: HostOs = lx.device.info.osName;
const applinkScene: AppLaunchScene = 8003;
const launchOptions: AppLaunchOptions = { scene: applinkScene };
// @ts-expect-error binding-layer class names are not part of the public contract
type _NoJsVideoContext = import("../src/index.js").JSVideoContext;
const videoSize: number = videoInfo.size;
const videoPath: string = videoInfo.path;
const videoCodec: string | undefined = videoInfo.videoCodec;
const hasAudio: boolean | undefined = videoInfo.hasAudio;
const audioCodec: string | undefined = videoInfo.audioCodec;

type AppPathIsBranded = Assert<Not<string extends AppDownloadFilePath ? true : false>>;
type DownloadsPathIsBranded = Assert<Not<string extends SystemDownloadsPath ? true : false>>;
type BrandsStayDistinct = Assert<Not<AppDownloadFilePath extends SystemDownloadsPath ? true : false>>;

export type GeneratedQualityGate = [
  typeof resolvedAppearance,
  typeof appearanceUnwatch,
  typeof appearancePreference,
  typeof appearanceSetResult,
  typeof navigationUpdateResult,
  typeof tabBarUpdateResult,
  typeof forcedTabBarUpdateResult,
  typeof backDefaultResult,
  typeof backCountResult,
  typeof pushed,
  typeof replaced,
  typeof tabbed,
  typeof otherApp,
  typeof appDownloadResult,
  typeof downloadsResult,
  typeof previewResult,
  typeof textResult,
  typeof binaryResult,
  typeof screenshotResult,
  typeof clipboardWrite,
  typeof clipboardText,
  typeof clipboardTypes,
  typeof cacheSize,
  typeof hostOs,
  typeof deviceOs,
  typeof launchOptions,
  _NoJsVideoContext,
  typeof videoSize,
  typeof videoPath,
  typeof videoCodec,
  typeof hasAudio,
  typeof audioCodec,
  AppPathIsBranded,
  DownloadsPathIsBranded,
  BrandsStayDistinct,
];
