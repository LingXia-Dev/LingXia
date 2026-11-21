import { errorMessage } from "../../shared/lib/errors";
import type {
  PickMediaOptions,
  PickedFile,
  CompressVideoTask,
  CompressImageOptions,
  CompressVideoOptions,
  VideoThumbnailOptions,
  ImageInfo,
  PreviewMediaOptions,
  PreviewMediaResult,
  PickCodeOptions,
  VideoCompressQuality,
  VideoInfo,
} from "@lingxia/types";
import { eventDetail, type NativeEvent } from "../../shared/lib/native-events";
import { definePage } from '@lingxia/types/page';

/** A labelled choice the page offers through an action sheet. */
interface MediaOption {
  key: string;
  label: string;
  value?: unknown;
  request?: string[];
}

/** One picked item, plus the video metadata the page enriches it with. */
interface ChosenMedia {
  path: string;
  type: string;
  displayWidth?: number;
  displayHeight?: number;
  displayAspectRatio?: string;
}

type MediaTypeToken = "image" | "video";

const DEFAULT_MODE = "Pictures";

const SOURCE_OPTIONS = [
  { key: "album", label: "Album", request: ["album"] },
  { key: "camera", label: "Camera", request: ["camera"] },
  { key: "either", label: "Album or Camera", request: ["album", "camera"] },
];

const COUNT_OPTIONS = Array.from({ length: 9 }, (_, index) => {
  const value = index + 1;
  return {
    key: String(value),
    label: String(value),
    value,
  };
});

const CAMERA_OPTIONS = [
  { key: "back", label: "Rear Camera" },
  { key: "front", label: "Front Camera" },
];

const DURATION_OPTIONS = [
  { key: "15", label: "15 seconds", value: 15 },
  { key: "30", label: "30 seconds", value: 30 },
  { key: "60", label: "60 seconds", value: 60 },
];

const ROTATE_OPTIONS = [
  { key: "meta", label: "Meta (Default)", value: undefined },
  { key: "0", label: "0°", value: 0 },
  { key: "90", label: "90°", value: 90 },
  { key: "180", label: "180°", value: 180 },
  { key: "270", label: "270°", value: 270 },
];

const OBJECT_FIT_OPTIONS = [
  { key: "default", label: "Default (Optional)", value: undefined },
  { key: "contain", label: "contain", value: "contain" },
  { key: "cover", label: "cover", value: "cover" },
  { key: "fill", label: "fill", value: "fill" },
  { key: "fit", label: "fit", value: "fit" },
];

const PREVIEW_BEHAVIOR_OPTIONS = [
  { key: "manual", label: "Manual Only" },
  { key: "next", label: "Auto Next" },
  { key: "loop", label: "Loop" },
];

function extractInputValue(event: NativeEvent<{ value: string }>) {
  return String(eventDetail(event).value ?? "");
}

function parseQualityInput(value: unknown) {
  const parsed = parseInt(String(value ?? ""), 10);
  return Number.isNaN(parsed) ? 80 : parsed;
}

function parsePositiveInt(value: unknown) {
  const parsed = parseInt(String(value ?? ""), 10);
  if (Number.isNaN(parsed) || parsed <= 0) {
    return undefined;
  }
  return parsed;
}

function kbpsToBps(kbps: number | undefined) {
  return kbps === undefined ? undefined : kbps * 1000;
}

function parseNonNegativeInt(value: unknown) {
  const parsed = parseInt(String(value ?? ""), 10);
  if (Number.isNaN(parsed) || parsed < 0) {
    return undefined;
  }
  return parsed;
}

function parseResolutionRatio(value: unknown) {
  const parsed = parseFloat(String(value ?? ""));
  if (!Number.isFinite(parsed) || parsed <= 0 || parsed > 1) {
    return undefined;
  }
  return parsed;
}

function resolveRotateValue(key: string) {
  const matched = ROTATE_OPTIONS.find((option) => option.key === key);
  if (!matched) return undefined;
  return typeof matched.value === "number" ? matched.value : undefined;
}

function resolveObjectFitValue(key: string, fallback = "contain") {
  const matched = OBJECT_FIT_OPTIONS.find((option) => option.key === key);
  if (!matched) return fallback;
  return typeof matched.value === "string" ? matched.value : undefined;
}

const MODE_SETTINGS = {
  Pictures: {
    mediaType: "image",
    defaults: {
      sourceKey: "album",
      countKey: "9",
    },
  },
  Videos: {
    mediaType: "video",
    defaults: {
      sourceKey: "album",
      cameraKey: "back",
      durationKey: "60",
      countKey: "3",
    },
  },
  ScanCode: {
    mediaType: "scanCode",
    defaults: {
      scanOnlyCamera: true,
      scanTypeKey: "all",
    },
  },
  ImageInfo: {
    mediaType: "imageInfo",
    defaults: {
      compressQuality: "80",
    },
  },
  VideoTools: {
    mediaType: "videoTools",
    defaults: {
      thumbnailQuality: "80",
      thumbnailTimeMs: "0",
    },
  },
  SaveToAlbum: {
    mediaType: "saveToAlbum",
    defaults: {},
  },
};

function getModeCopy(mediaType: string) {
  if (mediaType === "video") {
    return {
      headerSubtitle: "lx.pick.media / lx.media.preview",
      emptyHint: "Tap + to pick videos.",
      previewHint: "Tap Preview to open the full selection.",
      galleryHint: "Tap Preview to open the full selection.",
      addLabel: "Add Videos",
    };
  }
  if (mediaType === "imageInfo") {
    return {
      headerSubtitle: "lx.media.imageInfo / lx.media.compressImage",
      emptyHint: "Get image info and create compressed copy.",
      previewHint: "",
      galleryHint: "",
      addLabel: "Image Info",
    };
  }
  if (mediaType === "videoTools") {
    return {
      headerSubtitle: "lx.media.videoInfo / lx.media.videoThumbnail / lx.media.compressVideo",
      emptyHint: "Pick one video, inspect upload metadata, generate a thumbnail, and compress.",
      previewHint: "",
      galleryHint: "",
      addLabel: "Video Tools",
    };
  }
  if (mediaType === "saveToAlbum") {
    return {
      headerSubtitle: "lx.media.saveToLibrary",
      emptyHint: "Capture or select media to save to album.",
      previewHint: "",
      galleryHint: "",
      addLabel: "Save to Album",
    };
  }
  if (mediaType === "scanCode") {
    return {
      headerSubtitle: "lx.pick.code",
      emptyHint: "",
      previewHint: "",
      galleryHint: "",
      addLabel: "ScanCode",
    };
  }
  return {
    headerSubtitle: "lx.pick.media / lx.media.preview",
    emptyHint: "Tap + to pick photos.",
    previewHint: "Tap Preview to open the full selection.",
    galleryHint: "Tap Preview to open the full selection.",
    addLabel: "Add Photo",
  };
}

function getModeTitle(mediaType: string) {
  switch (mediaType) {
    case "video":
      return "Record / Pick Video";
    case "scanCode":
      return "Scan";
    case "imageInfo":
      return "Image Info";
    case "videoTools":
      return "Video Tools";
    case "saveToAlbum":
      return "Save to Album";
    default:
      return "Photos";
  }
}

function resolveModeKey(input: unknown): string {
  if (typeof input === "string" && input.trim()) {
    const normalized = input.trim().toLowerCase();
    const aliases: Record<string, string> = {
      videoinfo: "VideoTools",
      videothumbnail: "VideoTools",
      videotools: "VideoTools",
    };
    if (aliases[normalized]) {
      return aliases[normalized];
    }
    const matched = Object.keys(MODE_SETTINGS).find(
      (key) => key.toLowerCase() === normalized,
    );
    return matched || DEFAULT_MODE;
  }

  if (input && typeof input === "object") {
    // Support both type/mode fields
    const source = input as { type?: unknown; mode?: unknown };
    return resolveModeKey(source.type || source.mode);
  }

  return DEFAULT_MODE;
}

function findOption(options: MediaOption[], key: string | undefined, fallback?: MediaOption) {
  return options.find((option) => option.key === key) || fallback || options[0];
}

function resolveMediaTypeTokens(input: unknown): MediaTypeToken[] {
  if (input === "video") return ["video"];
  if (input === "image") return ["image"];
  return ["image", "video"];
}

function isCameraOnlySource(sourceOption: MediaOption | undefined) {
  const sources = sourceOption?.request || ["album"];
  return sources.includes("camera") && !sources.includes("album");
}

function mapChosenMedia(results: PickedFile[]): ChosenMedia[] {
  return results.map((item) => ({
    path: item.path,
    type: item.type.startsWith("video/") ? "video" : "image",
  }));
}

function resolveVideoDisplaySize(info: VideoInfo | undefined) {
  if (!info || !info.width || !info.height) return null;
  return {
    displayWidth: info.width,
    displayHeight: info.height,
    displayAspectRatio: `${info.width} / ${info.height}`,
  };
}

async function enrichVideoItemsWithMetadata(items: ChosenMedia[]): Promise<ChosenMedia[]> {
  return Promise.all(
    items.map(async (item) => {
      if (item.type !== "video") return item;
      const info = await lx.media.videoInfo({ path: item.path });
      const display = resolveVideoDisplaySize(info);
      return display ? { ...item, ...display } : item;
    }),
  );
}

async function pickOption(options: MediaOption[], currentKey?: string) {
  const result = await lx.ui.actionSheet({
    items: options.map((option, index) => ({ id: String(index), label: option.label })),
  });
  return (result.status === 'canceled') ? null : options[Number(result.id)] || null;
}

function createState(modeKey: unknown) {
  const key = resolveModeKey(modeKey);
  const settings = MODE_SETTINGS as unknown as Record<
    string,
    { mediaType: string; defaults?: Record<string, string | boolean> }
  >;
  const config = settings[key] || settings[DEFAULT_MODE];
  const defaults = config.defaults || {};

  const sourceOption = findOption(
    SOURCE_OPTIONS,
    defaults.sourceKey as string | undefined,
    SOURCE_OPTIONS[0],
  );
  const countOption = findOption(
    COUNT_OPTIONS,
    defaults.countKey as string | undefined,
    COUNT_OPTIONS[COUNT_OPTIONS.length - 1],
  );
  const cameraOption = findOption(
    CAMERA_OPTIONS,
    defaults.cameraKey as string | undefined,
    CAMERA_OPTIONS[0],
  );
  const durationOption = findOption(
    DURATION_OPTIONS,
    defaults.durationKey as string | undefined,
    DURATION_OPTIONS[DURATION_OPTIONS.length - 1],
  );

  const copy = getModeCopy(config.mediaType);

  return {
    modeKey: key,
    mediaType: config.mediaType,
    isRunning: false,
    selectedMedia: [] as ChosenMedia[],
    sourceKey: sourceOption ? sourceOption.key : "",
    countKey: countOption ? countOption.key : "",
    countLimit: countOption ? countOption.value : 0,
    cameraKey: cameraOption ? cameraOption.key : "",
    durationKey: durationOption ? durationOption.key : "",
    durationValue: durationOption ? durationOption.value : 0,
    scanOnlyCamera:
      config.mediaType === "scanCode" ? !!defaults.scanOnlyCamera : true,
    scanTypeKey:
      config.mediaType === "scanCode" ? defaults.scanTypeKey || "all" : "all",
    emptyHint: copy.emptyHint,
    previewHint: copy.previewHint,
    galleryHint: copy.galleryHint,
    headerSubtitle: copy.headerSubtitle,
    addLabel: copy.addLabel,
    scanResult: "",
    scanType: "",
    scanBusy: false,
    imageInfoResult: null as (ImageInfo & { path?: string; size?: number }) | null,
    imageInfoError: "",
    imageInfoBusy: false,
    compressQuality: defaults.compressQuality || "80",
    compressedWidth: "",
    compressedHeight: "",
    compressing: false,
    compressResult: null as
      | { path?: string; width?: number; height?: number; type?: string; size?: number }
      | null,
    compressError: "",
    videoInfoResult: null as VideoInfo | null,
    videoInfoError: "",
    videoInfoBusy: false,
    thumbnailVideoPath: "",
    thumbnailSourceInfo: null as VideoInfo | null,
    thumbnailQuality: defaults.thumbnailQuality || "80",
    thumbnailMaxWidth: "",
    thumbnailMaxHeight: "",
    thumbnailTimeMs: defaults.thumbnailTimeMs || "0",
    thumbnailBusy: false,
    thumbnailResult: null as { uri?: string } | null,
    thumbnailError: "",
    saveToAlbumBusy: false,
    videoCompressQuality: "medium",
    videoCompressBitrate: "1200",
    videoCompressFps: "30",
    videoCompressResolution: "0.8",
    videoCompressBusy: false,
    videoCompressProgress: null as number | null,
    videoCompressResult: null as { uri?: string } | null,
    videoCompressError: "",
    previewRotateKey: "meta",
    previewObjectFitKey: "default",
    previewBehaviorKey: "next",
    previewHideIndexIndicator: false,
    previewImageDurationMs: "2000",
    previewSessionBusy: false,
    previewSessionResult: null as PreviewMediaResult | null,
    previewSessionError: "",
    componentObjectFitKey: "cover",
  };
}

export default definePage({
  data: createState(DEFAULT_MODE),
  setup(page) {
    let previewAbortController = null as AbortController | null;
    // The live compression task, kept off `data`: it is a handle, not state.
    let compressVideoTask = null as CompressVideoTask | null;
    const helpers = {
      switchMode(params: unknown) {
        const modeKey = resolveModeKey(params);
        const state = createState(modeKey);
        page.update(d => { Object.assign(d, state); });

        const title = getModeTitle(state.mediaType);
        void page.navigationBar.update({ title });
      },
      async runPreviewSession({
        sources,
        startIndex,
        advance,
      }: {
        sources: (string | { path: string; type?: string })[];
        startIndex?: number;
        advance?: string;
      }) {
        actions.cancelPreviewSession();
        const controller = new AbortController();
        previewAbortController = controller;
        // Callers may pass sources as plain path strings (thumbnail / compressed
        // image previews) or as `{ path, type, ... }` objects (multi-select).
        // Normalize to objects so spreading a single source can't scatter a bare
        // string into character-indexed props (which drops `path`).
        const normalizedSources = sources.map((source) =>
          typeof source === "string" ? { path: source } : source
        );
        const request = (normalizedSources.length === 1
          ? { ...normalizedSources[0], advance, signal: controller.signal }
          : { sources: normalizedSources, startIndex, advance, signal: controller.signal }) as PreviewMediaOptions & Record<string, unknown>;
        if (page.data.previewHideIndexIndicator) {
          request.showIndexIndicator = false;
        }

        page.update(d => {
          d.previewSessionBusy = true;
          d.previewSessionResult = null;
          d.previewSessionError = "";
        });

        try {
          const handle = lx.media.preview(request);
          // Telemetry: log first-paint latency. The showcase doesn't have an
          // underlying overlay to hide, so we just observe and log.
          const startedAt = Date.now();
          handle.presented.then((outcome) => {
            if (outcome.status !== "presented") return;
            const ms = Date.now() - startedAt;
            console.log("[media-demo] presented:", { latencyMs: ms });
          });
          // Live change stream: know which item the user is looking at, as
          // they swipe / the session auto-advances.
          const unsubscribe = handle.onChange(({ index, source }) => {
            console.log("[media-demo] viewing:", index, source.path);
          });
          const result = await handle.completed;
          unsubscribe();
          page.update(d => {
            d.previewSessionBusy = false;
            d.previewSessionResult = result;
            d.previewSessionError = "";
          });
          return result;
        } catch (error) {
          const abortLike = error as { name?: string; code?: string } | null;
          const isAbort = abortLike?.name === "AbortError" || abortLike?.code === "E_ABORT";
          page.update(d => {
            d.previewSessionBusy = false;
            d.previewSessionError = isAbort ? "" : (errorMessage(error, "Preview failed"));
          });
          if (!isAbort) {
            console.error("[media-demo] media.preview failed:", error);
            lx.ui.toast({ title: errorMessage(error, "Preview failed"), icon: "none" });
          }
          return null;
        } finally {
          if (previewAbortController === controller) {
            previewAbortController = null;
          }
        }
      },
      async getFileSize(path: string) {
        try {
          const stat = await lx.fs.file(path).stat();
          return stat.size || 0;
        } catch (error) {
          console.warn("Failed to get file size:", error);
          return 0;
        }
      },
      async pickSingleMedia(type: string) {
        try {
          const result = await lx.pick.media({
            count: 1,
            mediaType: [type === "video" ? "video" : "image"],
            sourceType: ["album", "camera"],
            camera: "back",
          });
          return (result.status === 'canceled') ? null : result.files[0].path;
        } catch (error) {
          console.error("[media-demo] pickSingleMedia failed:", error);
          lx.ui.toast({ title: errorMessage(error, "pick.media failed"), icon: "none" });
          return null;
        }
      },
    };
    const actions = {
      async openSourcePicker() {
        const choice = await pickOption(SOURCE_OPTIONS, page.data.sourceKey);
        if (!choice) {
          return;
        }
        const sourceChanged = choice.key !== page.data.sourceKey;
        const cameraOnly = isCameraOnlySource(choice);
        const updates: {
          sourceKey: string;
          selectedMedia?: ChosenMedia[];
          countKey?: string;
          countLimit?: number;
        } = {
          sourceKey: choice.key,
        };

        if (sourceChanged) {
          updates.selectedMedia = [];
        }

        if (
          (page.data.mediaType === "image" || page.data.mediaType === "video") &&
          cameraOnly
        ) {
          updates.countKey = "1";
          updates.countLimit = 1;
        }

        page.update(d => { Object.assign(d, updates); });
      },
      async openCountPicker() {
        if (!COUNT_OPTIONS.length) {
          return;
        }
        const currentCountKey =
          COUNT_OPTIONS.find((option) => option.value === page.data.countLimit)?.key ||
          page.data.countKey;
        const choice = await pickOption(COUNT_OPTIONS, currentCountKey);
        if (!choice) {
          return;
        }
        page.update(d => {
          d.countKey = choice.key;
          d.countLimit = choice.value;
        });
      },
      async openCameraPicker() {
        if (!CAMERA_OPTIONS.length) {
          return;
        }
        const choice = await pickOption(CAMERA_OPTIONS, page.data.cameraKey);
        if (!choice) {
          return;
        }
        page.update(d => { d.cameraKey = choice.key; });
      },
      async openDurationPicker() {
        if (!DURATION_OPTIONS.length) {
          return;
        }
        const choice = await pickOption(DURATION_OPTIONS, page.data.durationKey);
        if (!choice) {
          return;
        }
        page.update(d => {
          d.durationKey = choice.key;
          d.durationValue = choice.value;
        });
      },
      async openPreviewRotatePicker() {
        const choice = await pickOption(ROTATE_OPTIONS, page.data.previewRotateKey || "meta");
        if (!choice) return;
        page.update(d => { d.previewRotateKey = choice.key; });
      },
      async openPreviewObjectFitPicker() {
        const choice = await pickOption(OBJECT_FIT_OPTIONS, page.data.previewObjectFitKey || "default");
        if (!choice) return;
        page.update(d => { d.previewObjectFitKey = choice.key; });
      },
      async openPreviewBehaviorPicker() {
        const choice = await pickOption(
          PREVIEW_BEHAVIOR_OPTIONS,
          page.data.previewBehaviorKey || "manual",
        );
        if (!choice) return;
        page.update(d => { d.previewBehaviorKey = choice.key; });
      },
      togglePreviewIndexIndicator() {
        page.update(d => { d.previewHideIndexIndicator = !page.data.previewHideIndexIndicator; });
      },
      onPreviewImageDurationInput(event: NativeEvent<{ value: string }>) {
        page.update(d => { d.previewImageDurationMs = extractInputValue(event); });
      },
      async openComponentObjectFitPicker() {
        const choice = await pickOption(OBJECT_FIT_OPTIONS, page.data.componentObjectFitKey || "cover");
        if (!choice) return;
        page.update(d => { d.componentObjectFitKey = choice.key; });
      },
      cancelPreviewSession() {
        if (previewAbortController) {
          const controller = previewAbortController;
          previewAbortController = null;
          try {
            controller.abort();
          } catch (error) {
            console.warn("[media-demo] preview abort failed:", error);
          }
        }
      },
      async launchMediaDemo() {
        if (page.data.mediaType === "scanCode") {
          actions.startScan();
          return;
        }
        if (page.data.isRunning) return;

        const sourceOption = findOption(
          SOURCE_OPTIONS,
          page.data.sourceKey,
          SOURCE_OPTIONS[0],
        );
        const cameraOnly = isCameraOnlySource(sourceOption);
        const countLimit = parsePositiveInt(page.data.countLimit || page.data.countKey);

        const request: PickMediaOptions = {
          mediaType: resolveMediaTypeTokens(page.data.mediaType),
          sourceType: (sourceOption?.request as PickMediaOptions["sourceType"]) || ["album"],
        };
        if (cameraOnly) {
          request.count = 1;
        } else if (typeof countLimit === "number" && countLimit > 0) {
          request.count = countLimit;
        }
        if (page.data.mediaType === "video") {
          const maxDurationSeconds = parsePositiveInt(page.data.durationValue);
          if (maxDurationSeconds) request.maxDurationSeconds = maxDurationSeconds;
          if (page.data.cameraKey) request.camera = page.data.cameraKey as PickMediaOptions["camera"];
        }

        page.update(d => {
          d.isRunning = true;
          d.selectedMedia = [];
        });

        try {
          const picked = await lx.pick.media(request);
          if (picked.status === 'canceled') {
            return;
          }
          const mapped = mapChosenMedia(picked.files);
          const enrichedList =
            page.data.mediaType === "video"
              ? await enrichVideoItemsWithMetadata(mapped)
              : mapped;
          page.update(d => { d.selectedMedia = enrichedList; });
        } catch (error) {
          console.error("[media-demo] pick.media failed:", error);
          lx.ui.toast({
            title: errorMessage(error, "Operation failed"),
            icon: "none",
          });
        } finally {
          page.update(d => { d.isRunning = false; });
        }
      },
      async startScan() {
        if (page.data.scanBusy) {
          return;
        }
        page.update(d => {
          d.scanBusy = true;
          d.scanResult = "";
          d.scanType = "";
        });
        try {
          const onlyFromCamera = !!page.data.scanOnlyCamera;
          const scanTypeKey = page.data.scanTypeKey || "all";
          const payload: PickCodeOptions = { onlyFromCamera };
          if (scanTypeKey && scanTypeKey !== "all") {
            payload.formats = [scanTypeKey as NonNullable<PickCodeOptions["formats"]>[number]];
          }
          const result = await lx.pick.code(payload);
          if (result.status === 'canceled') {
            page.update(d => { d.scanBusy = false; });
            return;
          }
          page.update(d => {
            d.scanBusy = false;
            d.scanResult = result.value;
            d.scanType = result.format;
          });
        } catch (error) {
          console.error("pick.code failed:", error);
          page.update(d => { d.scanBusy = false; });
          lx.ui.toast({
            title: errorMessage(error, "pick.code failed"),
            icon: "none",
          });
        }
      },
      async openScanSourcePicker() {
        const OPTIONS = [
          { key: "camera", label: "Camera Only", value: true },
          { key: "cameraOrPhoto", label: "Camera & Photo", value: false },
        ];
        const currentKey = page.data.scanOnlyCamera ? "camera" : "cameraOrPhoto";
        const choice = await pickOption(OPTIONS, currentKey);
        if (!choice) return;
        page.update(d => { d.scanOnlyCamera = choice.key === "camera"; });
      },
      async openScanTypePicker() {
        const OPTIONS = [
          { key: "all", label: "All" },
          { key: "barCode", label: "barCode (1D)" },
          { key: "qrCode", label: "qrCode" },
          { key: "datamatrix", label: "datamatrix" },
          { key: "pdf417", label: "pdf417" },
        ];
        const choice = await pickOption(OPTIONS, String(page.data.scanTypeKey || "all"));
        if (!choice) return;
        page.update(d => { d.scanTypeKey = choice.key; });
      },
      async previewSelectedMedia() {
        const selected = page.data.selectedMedia || [];
        if (!selected.length) {
          lx.ui.toast({
            title: "Nothing to preview",
            icon: "none",
          });
          return;
        }

        const previewRotate = resolveRotateValue(page.data.previewRotateKey || "meta");
        const previewObjectFit = resolveObjectFitValue(page.data.previewObjectFitKey || "default", "contain");
        const advance = page.data.previewBehaviorKey || "manual";
        const imageDurationMs = parsePositiveInt(page.data.previewImageDurationMs);
        const sources = selected.map((item) => {
          const source: {
            path: string;
            type: string;
            rotate?: number;
            objectFit?: string;
            durationMs?: number;
          } = { path: item.path, type: item.type };
          if (typeof previewRotate === "number") source.rotate = previewRotate;
          if (previewObjectFit) source.objectFit = previewObjectFit;
          if (item.type !== "video" && (imageDurationMs ?? 0) > 0) source.durationMs = imageDurationMs;
          return source;
        });
        await helpers.runPreviewSession({
          sources,
          startIndex: 0,
          advance,
        });
      },
      async pickImageForInfo() {
        if (page.data.imageInfoBusy) {
          return;
        }
        const picked = await helpers.pickSingleMedia("image");
        if (!picked) {
          return;
        }
        page.update(d => {
          d.imageInfoBusy = true;
          d.imageInfoError = "";
          d.imageInfoResult = null;
        });
        try {
          const info = await lx.media.imageInfo({ path: picked });
          const size = await helpers.getFileSize(picked);
          page.update(d => {
            d.imageInfoResult = { ...info, size };
            d.imageInfoBusy = false;
          });
        } catch (error) {
          const message = errorMessage(error, "media.imageInfo failed");
          page.update(d => {
            d.imageInfoError = message;
            d.imageInfoResult = null;
            d.imageInfoBusy = false;
          });
          lx.ui.toast({
            title: message,
            icon: "none",
          });
        }
      },
      async pickVideoForTools() {
        if (page.data.thumbnailBusy || page.data.videoInfoBusy) {
          return;
        }
        const picked = await helpers.pickSingleMedia("video");
        if (!picked) {
          return;
        }
        page.update(d => {
          d.thumbnailVideoPath = picked;
          d.thumbnailSourceInfo = null;
          d.thumbnailError = "";
          d.thumbnailResult = null;
          d.videoCompressError = "";
          d.videoCompressResult = null;
          d.videoInfoBusy = true;
          d.videoInfoError = "";
          d.videoInfoResult = null;
        });
        try {
          const info = await lx.media.videoInfo({ path: picked });
          page.update(d => {
            d.videoInfoResult = info;
            d.videoInfoBusy = false;
            d.thumbnailSourceInfo = info;
          });
        } catch (error) {
          const message = errorMessage(error, "media.videoInfo failed");
          page.update(d => {
            d.videoInfoBusy = false;
            d.videoInfoError = message;
          });
          lx.ui.toast({
            title: message,
            icon: "none",
          });
        }
      },
      onThumbnailQualityInput(event: NativeEvent<{ value: string }>) {
        const value = extractInputValue(event);
        page.update(d => { d.thumbnailQuality = value; });
      },
      onThumbnailMaxWidthInput(event: NativeEvent<{ value: string }>) {
        const value = extractInputValue(event);
        page.update(d => { d.thumbnailMaxWidth = value; });
      },
      onThumbnailMaxHeightInput(event: NativeEvent<{ value: string }>) {
        const value = extractInputValue(event);
        page.update(d => { d.thumbnailMaxHeight = value; });
      },
      onThumbnailTimeInput(event: NativeEvent<{ value: string }>) {
        const value = extractInputValue(event);
        page.update(d => { d.thumbnailTimeMs = value; });
      },
      async createVideoThumbnail() {
        if (page.data.thumbnailBusy) {
          return;
        }
        const sourcePath = page.data.thumbnailVideoPath;
        if (!sourcePath) {
          lx.ui.toast({
            title: "Please pick a video first",
            icon: "none",
          });
          return;
        }

        const quality = parseQualityInput(page.data.thumbnailQuality);
        const maxWidth = parsePositiveInt(page.data.thumbnailMaxWidth);
        const maxHeight = parsePositiveInt(page.data.thumbnailMaxHeight);
        const timeMs = parseNonNegativeInt(page.data.thumbnailTimeMs);

        const payload: VideoThumbnailOptions = {
          path: sourcePath,
          quality,
        };
        if (typeof maxWidth === "number") {
          payload.maxWidth = maxWidth;
        }
        if (typeof maxHeight === "number") {
          payload.maxHeight = maxHeight;
        }
        if (typeof timeMs === "number") {
          payload.timeMs = timeMs;
        }

        page.update(d => {
          d.thumbnailBusy = true;
          d.thumbnailError = "";
          d.thumbnailResult = null;
        });
        try {
          const thumbnail = await lx.media.videoThumbnail(payload);
          page.update(d => {
            d.thumbnailResult = thumbnail;
            d.thumbnailBusy = false;
          });
        } catch (error) {
          const message = errorMessage(error, "media.videoThumbnail failed");
          page.update(d => {
            d.thumbnailError = message;
            d.thumbnailResult = null;
            d.thumbnailBusy = false;
          });
          lx.ui.toast({
            title: message,
            icon: "none",
          });
        }
      },
      async previewVideoThumbnail() {
        const path = page.data.thumbnailResult?.uri;
        if (!path) {
          lx.ui.toast({
            title: "No thumbnail to preview",
            icon: "none",
          });
          return;
        }

        await helpers.runPreviewSession({
          sources: [path],
          startIndex: 0,
          advance: "manual",
        });
      },
      onVideoCompressQualityInput(event: NativeEvent<{ value: string }>) {
        const value = extractInputValue(event).trim().toLowerCase();
        page.update(d => { d.videoCompressQuality = value; });
      },
      onVideoCompressBitrateInput(event: NativeEvent<{ value: string }>) {
        const value = extractInputValue(event);
        page.update(d => { d.videoCompressBitrate = value; });
      },
      onVideoCompressFpsInput(event: NativeEvent<{ value: string }>) {
        const value = extractInputValue(event);
        page.update(d => { d.videoCompressFps = value; });
      },
      onVideoCompressResolutionInput(event: NativeEvent<{ value: string }>) {
        const value = extractInputValue(event);
        page.update(d => { d.videoCompressResolution = value; });
      },
      async compressSelectedVideo() {
        if (page.data.videoCompressBusy) {
          return;
        }
        const sourcePath = page.data.thumbnailVideoPath;
        if (!sourcePath) {
          lx.ui.toast({
            title: "Please pick a video first",
            icon: "none",
          });
          return;
        }

        const quality = (page.data.videoCompressQuality || "").trim().toLowerCase();
        const payload: CompressVideoOptions = quality
          ? { path: sourcePath, quality: quality as VideoCompressQuality }
          : {
              path: sourcePath,
              bitrate: kbpsToBps(parsePositiveInt(page.data.videoCompressBitrate)),
              fps: parsePositiveInt(page.data.videoCompressFps),
              resolution: parseResolutionRatio(page.data.videoCompressResolution),
            };

        page.update(d => {
          d.videoCompressBusy = true;
          d.videoCompressError = "";
          d.videoCompressResult = null;
          d.videoCompressProgress = 0;
        });

        const task = lx.media.compressVideo(payload);
        compressVideoTask = task;
        try {
          for await (const { progress } of task.progress) {
            page.update(d => { d.videoCompressProgress = Math.round(progress * 100); });
          }
          const result = await task.result;
          page.update(d => {
            d.videoCompressResult = result;
            d.videoCompressBusy = false;
            d.videoCompressProgress = null;
          });
        } catch (error) {
          const abortLike = error as { name?: string; code?: string } | null;
          const isAbort = abortLike?.name === "AbortError" || abortLike?.code === "E_ABORT";
          const message = errorMessage(error, "compressVideo failed");
          page.update(d => {
            d.videoCompressError = isAbort ? "" : message;
            d.videoCompressResult = null;
            d.videoCompressBusy = false;
            d.videoCompressProgress = null;
          });
          lx.ui.toast({
            title: isAbort ? "Compression cancelled" : message,
            icon: "none",
          });
        } finally {
          compressVideoTask = null;
        }
      },
      cancelVideoCompress() {
        compressVideoTask?.cancel();
      },
      async previewCompressedVideo() {
        const path = page.data.videoCompressResult?.uri;
        if (!path) {
          lx.ui.toast({ title: "No compressed video to preview", icon: "none" });
          return;
        }
        const source: { path: string; type: string; rotate?: number; objectFit?: string } = {
          path,
          type: "video",
        };
        const rotate = resolveRotateValue(page.data.previewRotateKey || "meta");
        const objectFit = resolveObjectFitValue(page.data.previewObjectFitKey || "default", "contain");
        if (typeof rotate === "number") source.rotate = rotate;
        if (objectFit) source.objectFit = objectFit;
        await helpers.runPreviewSession({
          sources: [source],
          startIndex: 0,
          advance: "manual",
        });
      },
      onCompressQualityInput(event: NativeEvent<{ value: string }>) {
        const value = extractInputValue(event);
        page.update(d => { d.compressQuality = value; });
      },
      onCompressedWidthInput(event: NativeEvent<{ value: string }>) {
        const value = extractInputValue(event);
        page.update(d => { d.compressedWidth = value; });
      },
      onCompressedHeightInput(event: NativeEvent<{ value: string }>) {
        const value = extractInputValue(event);
        page.update(d => { d.compressedHeight = value; });
      },
      async compressSelectedImage() {
        if (page.data.compressing) {
          return;
        }
        const path = page.data.imageInfoResult?.path;
        if (!path) {
          lx.ui.toast({
            title: "Please select an image first",
            icon: "none",
          });
          return;
        }

        const quality = parseQualityInput(page.data.compressQuality);
        const compressedWidth = parsePositiveInt(page.data.compressedWidth);
        const compressedHeight = parsePositiveInt(page.data.compressedHeight);

        const payload: CompressImageOptions = { path, quality };
        if (typeof compressedWidth === "number") {
          payload.compressedWidth = compressedWidth;
        }
        if (typeof compressedHeight === "number") {
          payload.compressedHeight = compressedHeight;
        }

        page.update(d => {
          d.compressing = true;
          d.compressError = "";
          d.compressResult = null;
        });

        try {
          const result = await lx.media.compressImage(payload);
          const resultPath = result.uri;
          const info = await lx.media.imageInfo({ path: resultPath });
          const size = await helpers.getFileSize(resultPath);
          page.update(d => {
            d.compressResult = { path: resultPath, width: info.width, height: info.height, type: info.type, size };
          });
        } catch (error) {
          const message = errorMessage(error, "compressImage failed");
          page.update(d => {
            d.compressError = message;
            d.compressResult = null;
          });
          lx.ui.toast({
            title: message,
            icon: "none",
          });
        } finally {
          page.update(d => { d.compressing = false; });
        }
      },
      async previewCompressedImage() {
        const path = page.data.compressResult?.path;
        if (!path) {
          lx.ui.toast({
            title: "No compressed image to preview",
            icon: "none",
          });
          return;
        }

        await helpers.runPreviewSession({
          sources: [path],
          startIndex: 0,
          advance: "manual",
        });
      },
      async captureImageForAlbum() {
        if (page.data.saveToAlbumBusy) return;
        page.update(d => { d.saveToAlbumBusy = true; });
        try {
          const result = await lx.pick.media({
            count: 1, mediaType: ["image"], sourceType: ["camera"], camera: "back",
          });
          if (result.status === 'canceled') return;
          await lx.media.saveToLibrary(result.files[0].path);
          lx.ui.toast({ title: "Image saved to album", icon: "success" });
        } catch (error) {
          lx.ui.toast({ title: errorMessage(error, "Failed to save image"), icon: "none" });
        } finally {
          page.update(d => { d.saveToAlbumBusy = false; });
        }
      },
      async captureVideoForAlbum() {
        if (page.data.saveToAlbumBusy) return;
        page.update(d => { d.saveToAlbumBusy = true; });
        try {
          const result = await lx.pick.media({
            count: 1, mediaType: ["video"], sourceType: ["camera"], camera: "back", maxDurationSeconds: 60,
          });
          if (result.status === 'canceled') return;
          await lx.media.saveToLibrary(result.files[0].path);
          lx.ui.toast({ title: "Video saved to album", icon: "success" });
        } catch (error) {
          lx.ui.toast({ title: errorMessage(error, "Failed to save video"), icon: "none" });
        } finally {
          page.update(d => { d.saveToAlbumBusy = false; });
        }
      },
    };
    return {
      onLoad(options) {
        helpers.switchMode(options?.type);
      },
      onHide() {
        actions.cancelPreviewSession();
        page.update(d => {
          d.selectedMedia = [];
          d.previewSessionBusy = false;
        });
      },
      actions,
    };
  },
});
