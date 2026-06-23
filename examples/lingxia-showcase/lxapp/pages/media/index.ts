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

function extractInputValue(event) {
  return String(event?.detail?.value ?? "");
}

function parseQualityInput(value) {
  const parsed = parseInt(String(value ?? ""), 10);
  return Number.isNaN(parsed) ? 80 : parsed;
}

function parsePositiveInt(value) {
  const parsed = parseInt(String(value ?? ""), 10);
  if (Number.isNaN(parsed) || parsed <= 0) {
    return undefined;
  }
  return parsed;
}

function parseNonNegativeInt(value) {
  const parsed = parseInt(String(value ?? ""), 10);
  if (Number.isNaN(parsed) || parsed < 0) {
    return undefined;
  }
  return parsed;
}

function parseResolutionRatio(value) {
  const parsed = parseFloat(String(value ?? ""));
  if (!Number.isFinite(parsed) || parsed <= 0 || parsed > 1) {
    return undefined;
  }
  return parsed;
}

function resolveRotateValue(key) {
  const matched = ROTATE_OPTIONS.find((option) => option.key === key);
  if (!matched) return undefined;
  return typeof matched.value === "number" ? matched.value : undefined;
}

function resolveObjectFitValue(key, fallback = "contain") {
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

function getModeCopy(mediaType) {
  if (mediaType === "video") {
    return {
      headerSubtitle: "lx.chooseMedia / lx.previewMedia",
      emptyHint: "Tap + to pick videos.",
      previewHint: "Tap Preview to open the full selection.",
      galleryHint: "Tap Preview to open the full selection.",
      addLabel: "Add Videos",
    };
  }
  if (mediaType === "imageInfo") {
    return {
      headerSubtitle: "lx.getImageInfo / lx.compressImage",
      emptyHint: "Get image info and create compressed copy.",
      previewHint: "",
      galleryHint: "",
      addLabel: "Image Info",
    };
  }
  if (mediaType === "videoTools") {
    return {
      headerSubtitle: "lx.getVideoInfo / lx.extractVideoThumbnail / lx.compressVideo",
      emptyHint: "Pick one video, inspect metadata, generate thumbnail, and compress.",
      previewHint: "",
      galleryHint: "",
      addLabel: "Video Tools",
    };
  }
  if (mediaType === "saveToAlbum") {
    return {
      headerSubtitle: "lx.saveImageToPhotosAlbum / lx.saveVideoToPhotosAlbum",
      emptyHint: "Capture or select media to save to album.",
      previewHint: "",
      galleryHint: "",
      addLabel: "Save to Album",
    };
  }
  if (mediaType === "scanCode") {
    return {
      headerSubtitle: "lx.scanCode",
      emptyHint: "",
      previewHint: "",
      galleryHint: "",
      addLabel: "ScanCode",
    };
  }
  return {
    headerSubtitle: "lx.chooseMedia / lx.previewMedia",
    emptyHint: "Tap + to pick photos.",
    previewHint: "Tap Preview to open the full selection.",
    galleryHint: "Tap Preview to open the full selection.",
    addLabel: "Add Photo",
  };
}

function getModeTitle(mediaType) {
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

function resolveModeKey(input) {
  if (typeof input === "string" && input.trim()) {
    const normalized = input.trim().toLowerCase();
    const aliases = {
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
    return resolveModeKey(input.type || input.mode);
  }

  return DEFAULT_MODE;
}

function findOption(options, key, fallback) {
  return options.find((option) => option.key === key) || fallback || options[0];
}

function resolveMediaTypeTokens(input) {
  if (input === "video") return ["video"];
  if (input === "image") return ["image"];
  return ["image", "video"];
}

function isCameraOnlySource(sourceOption) {
  const sources = sourceOption?.request || ["album"];
  return sources.includes("camera") && !sources.includes("album");
}

function mapChosenMedia(results) {
  return results.map((item) => ({
    path: item.tempFilePath,
    type: item.fileType,
  }));
}

function resolveVideoDisplaySize(info) {
  if (!info || !info.width || !info.height) return null;
  return {
    displayWidth: info.width,
    displayHeight: info.height,
    displayAspectRatio: `${info.width} / ${info.height}`,
  };
}

async function enrichVideoItemsWithMetadata(items) {
  return Promise.all(
    items.map(async (item) => {
      if (item.type !== "video") return item;
      const info = await lx.getVideoInfo({ path: item.path });
      const display = resolveVideoDisplaySize(info);
      return display ? { ...item, ...display } : item;
    }),
  );
}

async function pickOption(options, currentKey) {
  try {
    const result = await lx.showActionSheet({
      itemList: options.map((option) => option.label),
    });
    return options[result.tapIndex] || null;
  } catch (error) {
    // User cancelled
    return null;
  }
}

function createState(modeKey) {
  const key = resolveModeKey(modeKey);
  const config = MODE_SETTINGS[key] || MODE_SETTINGS[DEFAULT_MODE];
  const defaults = config.defaults || {};

  const sourceOption = findOption(
    SOURCE_OPTIONS,
    defaults.sourceKey,
    SOURCE_OPTIONS[0],
  );
  const countOption = findOption(
    COUNT_OPTIONS,
    defaults.countKey,
    COUNT_OPTIONS[COUNT_OPTIONS.length - 1],
  );
  const cameraOption = findOption(
    CAMERA_OPTIONS,
    defaults.cameraKey,
    CAMERA_OPTIONS[0],
  );
  const durationOption = findOption(
    DURATION_OPTIONS,
    defaults.durationKey,
    DURATION_OPTIONS[DURATION_OPTIONS.length - 1],
  );

  const copy = getModeCopy(config.mediaType);

  return {
    modeKey: key,
    mediaType: config.mediaType,
    isRunning: false,
    selectedMedia: [],
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
    imageInfoResult: null,
    imageInfoError: "",
    imageInfoBusy: false,
    compressQuality: defaults.compressQuality || "80",
    compressedWidth: "",
    compressedHeight: "",
    compressing: false,
    compressResult: null,
    compressError: "",
    videoInfoResult: null,
    videoInfoError: "",
    videoInfoBusy: false,
    thumbnailVideoPath: "",
    thumbnailSourceInfo: null,
    thumbnailQuality: defaults.thumbnailQuality || "80",
    thumbnailMaxWidth: "",
    thumbnailMaxHeight: "",
    thumbnailTimeMs: defaults.thumbnailTimeMs || "0",
    thumbnailBusy: false,
    thumbnailResult: null,
    thumbnailError: "",
    videoCompressQuality: "medium",
    videoCompressBitrate: "1200",
    videoCompressFps: "30",
    videoCompressResolution: "0.8",
    videoCompressBusy: false,
    videoCompressProgress: null,
    videoCompressResult: null,
    videoCompressError: "",
    previewRotateKey: "meta",
    previewObjectFitKey: "default",
    previewBehaviorKey: "next",
    previewHideIndexIndicator: false,
    previewImageDurationMs: "2000",
    previewSessionBusy: false,
    previewSessionResult: null,
    previewSessionError: "",
    componentRotateKey: "meta",
    componentObjectFitKey: "cover",
  };
}

Page({
  data: createState(DEFAULT_MODE),
  _previewAbortController: null,

  onLoad: function (options) {
    this._switchMode(options?.type);
  },

  onHide: function () {
    this.cancelPreviewSession();
    this.setData({
      selectedMedia: [],
      previewSessionBusy: false,
    });
  },

  _switchMode: function (params) {
    const modeKey = resolveModeKey(params);
    const state = createState(modeKey);
    this.setData(state);

    const title = getModeTitle(state.mediaType);
    lx.setNavigationBarTitle({ title });
  },

  openSourcePicker: async function () {
    const choice = await pickOption(SOURCE_OPTIONS, this.data.sourceKey);
    if (!choice) {
      return;
    }
    const sourceChanged = choice.key !== this.data.sourceKey;
    const cameraOnly = isCameraOnlySource(choice);
    const updates = {
      sourceKey: choice.key,
    };

    if (sourceChanged) {
      updates.selectedMedia = [];
    }

    if (
      (this.data.mediaType === "image" || this.data.mediaType === "video") &&
      cameraOnly
    ) {
      updates.countKey = "1";
      updates.countLimit = 1;
    }

    this.setData(updates);
  },

  openCountPicker: async function () {
    if (!COUNT_OPTIONS.length) {
      return;
    }
    const currentCountKey =
      COUNT_OPTIONS.find((option) => option.value === this.data.countLimit)?.key ||
      this.data.countKey;
    const choice = await pickOption(COUNT_OPTIONS, currentCountKey);
    if (!choice) {
      return;
    }
    this.setData({
      countKey: choice.key,
      countLimit: choice.value,
    });
  },

  openCameraPicker: async function () {
    if (!CAMERA_OPTIONS.length) {
      return;
    }
    const choice = await pickOption(CAMERA_OPTIONS, this.data.cameraKey);
    if (!choice) {
      return;
    }
    this.setData({
      cameraKey: choice.key,
    });
  },

  openDurationPicker: async function () {
    if (!DURATION_OPTIONS.length) {
      return;
    }
    const choice = await pickOption(DURATION_OPTIONS, this.data.durationKey);
    if (!choice) {
      return;
    }
    this.setData({
      durationKey: choice.key,
      durationValue: choice.value,
    });
  },

  openPreviewRotatePicker: async function () {
    const choice = await pickOption(ROTATE_OPTIONS, this.data.previewRotateKey || "meta");
    if (!choice) return;
    this.setData({ previewRotateKey: choice.key });
  },

  openPreviewObjectFitPicker: async function () {
    const choice = await pickOption(OBJECT_FIT_OPTIONS, this.data.previewObjectFitKey || "default");
    if (!choice) return;
    this.setData({ previewObjectFitKey: choice.key });
  },

  openPreviewBehaviorPicker: async function () {
    const choice = await pickOption(
      PREVIEW_BEHAVIOR_OPTIONS,
      this.data.previewBehaviorKey || "manual",
    );
    if (!choice) return;
    this.setData({ previewBehaviorKey: choice.key });
  },

  togglePreviewIndexIndicator: function () {
    this.setData({
      previewHideIndexIndicator: !this.data.previewHideIndexIndicator,
    });
  },

  onPreviewImageDurationInput: function (event) {
    this.setData({
      previewImageDurationMs: extractInputValue(event),
    });
  },

  openComponentRotatePicker: async function () {
    const choice = await pickOption(ROTATE_OPTIONS, this.data.componentRotateKey || "meta");
    if (!choice) return;
    this.setData({ componentRotateKey: choice.key });
  },

  openComponentObjectFitPicker: async function () {
    const choice = await pickOption(OBJECT_FIT_OPTIONS, this.data.componentObjectFitKey || "cover");
    if (!choice) return;
    this.setData({ componentObjectFitKey: choice.key });
  },

  cancelPreviewSession: function () {
    if (this._previewAbortController) {
      const controller = this._previewAbortController;
      this._previewAbortController = null;
      try {
        controller.abort();
      } catch (error) {
        console.warn("[media-demo] preview abort failed:", error);
      }
    }
  },

  launchMediaDemo: async function () {
    if (this.data.mediaType === "scanCode") {
      this.startScan();
      return;
    }
    if (this.data.isRunning) return;

    const sourceOption = findOption(
      SOURCE_OPTIONS,
      this.data.sourceKey,
      SOURCE_OPTIONS[0],
    );
    const cameraOnly = isCameraOnlySource(sourceOption);
    const countLimit = parsePositiveInt(this.data.countLimit || this.data.countKey);

    const request = {
      mediaType: resolveMediaTypeTokens(this.data.mediaType),
      sourceType: sourceOption?.request || ["album"],
    };
    if (cameraOnly) {
      request.count = 1;
    } else if (typeof countLimit === "number" && countLimit > 0) {
      request.count = countLimit;
    }
    if (this.data.mediaType === "video") {
      if (this.data.durationValue > 0)
        request.maxDuration = this.data.durationValue;
      if (this.data.cameraKey) request.camera = this.data.cameraKey;
    }

    this.setData({ isRunning: true, selectedMedia: [] });

    try {
      const results = await lx.chooseMedia(request);
      const mapped = mapChosenMedia(results);
      const enrichedList =
        this.data.mediaType === "video"
          ? await enrichVideoItemsWithMetadata(mapped)
          : mapped;
      this.setData({ selectedMedia: enrichedList });
    } catch (error) {
      console.error("[media-demo] chooseMedia failed:", error);
      lx.showToast({
        title: error?.message || "Operation failed",
        icon: "none",
      });
    } finally {
      this.setData({ isRunning: false });
    }
  },

  startScan: async function () {
    if (this.data.scanBusy) {
      return;
    }
    this.setData({ scanBusy: true, scanResult: "", scanType: "" });
    try {
      const onlyFromCamera = !!this.data.scanOnlyCamera;
      const scanTypeKey = this.data.scanTypeKey || "all";
      const payload = { onlyFromCamera };
      if (scanTypeKey && scanTypeKey !== "all") {
        payload.scanType = [scanTypeKey];
      }
      const result = await lx.scanCode(payload);
      this.setData({ scanBusy: false, scanResult: result.scanResult, scanType: result.scanType });
    } catch (error) {
      console.error("scanCode failed:", error);
      this.setData({ scanBusy: false });
      lx.showToast({
        title: error?.message || "scanCode failed",
        icon: "none",
      });
    }
  },

  openScanSourcePicker: async function () {
    const OPTIONS = [
      { key: "camera", label: "Camera Only", value: true },
      { key: "cameraOrPhoto", label: "Camera & Photo", value: false },
    ];
    const currentKey = this.data.scanOnlyCamera ? "camera" : "cameraOrPhoto";
    const choice = await pickOption(OPTIONS, currentKey);
    if (!choice) return;
    this.setData({ scanOnlyCamera: choice.key === "camera" });
  },

  openScanTypePicker: async function () {
    const OPTIONS = [
      { key: "all", label: "All" },
      { key: "barCode", label: "barCode (1D)" },
      { key: "qrCode", label: "qrCode" },
      { key: "datamatrix", label: "datamatrix" },
      { key: "pdf417", label: "pdf417" },
    ];
    const choice = await pickOption(OPTIONS, this.data.scanTypeKey || "all");
    if (!choice) return;
    this.setData({ scanTypeKey: choice.key });
  },

  previewSelectedMedia: async function () {
    const selected = this.data.selectedMedia || [];
    if (!selected.length) {
      lx.showToast({
        title: "Nothing to preview",
        icon: "none",
      });
      return;
    }

    const previewRotate = resolveRotateValue(this.data.previewRotateKey || "meta");
    const previewObjectFit = resolveObjectFitValue(this.data.previewObjectFitKey || "default", "contain");
    const advance = this.data.previewBehaviorKey || "manual";
    const imageDurationMs = parsePositiveInt(this.data.previewImageDurationMs);
    const sources = selected.map((item) => {
      const source = { path: item.path, type: item.type };
      if (typeof previewRotate === "number") source.rotate = previewRotate;
      if (previewObjectFit) source.objectFit = previewObjectFit;
      if (item.type !== "video" && imageDurationMs > 0) source.durationMs = imageDurationMs;
      return source;
    });
    await this._runPreviewSession({
      sources,
      startIndex: 0,
      advance,
    });
  },

  _runPreviewSession: async function ({
    sources,
    startIndex,
    advance,
  }) {
    this.cancelPreviewSession();
    const controller = new AbortController();
    this._previewAbortController = controller;
    // Callers may pass sources as plain path strings (thumbnail / compressed
    // image previews) or as `{ path, type, ... }` objects (multi-select).
    // Normalize to objects so spreading a single source can't scatter a bare
    // string into character-indexed props (which drops `path`).
    const normalizedSources = sources.map((source) =>
      typeof source === "string" ? { path: source } : source
    );
    const request = normalizedSources.length === 1
      ? { ...normalizedSources[0], advance, signal: controller.signal }
      : { sources: normalizedSources, startIndex, advance, signal: controller.signal };
    if (this.data.previewHideIndexIndicator) {
      request.showIndexIndicator = false;
    }

    this.setData({ previewSessionBusy: true, previewSessionResult: null, previewSessionError: "" });

    try {
      const handle = lx.previewMedia(request);
      // Telemetry: log first-paint latency. The showcase doesn't have an
      // underlying overlay to hide, so we just observe and log.
      const startedAt = Date.now();
      handle.presented.then(() => {
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
      this.setData({ previewSessionBusy: false, previewSessionResult: result, previewSessionError: "" });
      return result;
    } catch (error) {
      const isAbort = error?.name === "AbortError";
      this.setData({
        previewSessionBusy: false,
        previewSessionError: isAbort ? "" : (error?.message || "Preview failed"),
      });
      if (!isAbort) {
        console.error("[media-demo] previewMedia failed:", error);
        lx.showToast({ title: error?.message || "Preview failed", icon: "none" });
      }
      return null;
    } finally {
      if (this._previewAbortController === controller) {
        this._previewAbortController = null;
      }
    }
  },

  pickImageForInfo: async function () {
    if (this.data.imageInfoBusy) {
      return;
    }
    const picked = await this._pickSingleMedia("image");
    if (!picked) {
      return;
    }
    this.setData({
      imageInfoBusy: true,
      imageInfoError: "",
      imageInfoResult: null,
    });
    try {
      const info = await lx.getImageInfo({ path: picked });
      const size = await this._getFileSize(picked);
      this.setData({ imageInfoResult: { ...info, size }, imageInfoBusy: false });
    } catch (error) {
      const message = error?.message || "getImageInfo failed";
      this.setData({
        imageInfoError: message,
        imageInfoResult: null,
        imageInfoBusy: false,
      });
      lx.showToast({
        title: message,
        icon: "none",
      });
    }
  },

  pickVideoForTools: async function () {
    if (this.data.thumbnailBusy || this.data.videoInfoBusy) {
      return;
    }
    const picked = await this._pickSingleMedia("video");
    if (!picked) {
      return;
    }
    this.setData({
      thumbnailVideoPath: picked,
      thumbnailSourceInfo: null,
      thumbnailError: "",
      thumbnailResult: null,
      videoCompressError: "",
      videoCompressResult: null,
      videoInfoBusy: true,
      videoInfoError: "",
      videoInfoResult: null,
    });
    try {
      const info = await lx.getVideoInfo({ path: picked });
      const size = await this._getFileSize(picked);
      this.setData({ videoInfoResult: { ...info, size }, videoInfoBusy: false, thumbnailSourceInfo: info });
    } catch (error) {
      const message = error?.message || "getVideoInfo failed";
      this.setData({
        videoInfoBusy: false,
        videoInfoError: message,
      });
      lx.showToast({
        title: message,
        icon: "none",
      });
    }
  },

  onThumbnailQualityInput: function (event) {
    const value = extractInputValue(event);
    this.setData({ thumbnailQuality: value });
  },

  onThumbnailMaxWidthInput: function (event) {
    const value = extractInputValue(event);
    this.setData({ thumbnailMaxWidth: value });
  },

  onThumbnailMaxHeightInput: function (event) {
    const value = extractInputValue(event);
    this.setData({ thumbnailMaxHeight: value });
  },

  onThumbnailTimeInput: function (event) {
    const value = extractInputValue(event);
    this.setData({ thumbnailTimeMs: value });
  },

  createVideoThumbnail: async function () {
    if (this.data.thumbnailBusy) {
      return;
    }
    const sourcePath = this.data.thumbnailVideoPath;
    if (!sourcePath) {
      lx.showToast({
        title: "Please pick a video first",
        icon: "none",
      });
      return;
    }

    const quality = parseQualityInput(this.data.thumbnailQuality);
    const maxWidth = parsePositiveInt(this.data.thumbnailMaxWidth);
    const maxHeight = parsePositiveInt(this.data.thumbnailMaxHeight);
    const timeMs = parseNonNegativeInt(this.data.thumbnailTimeMs);

    const payload = {
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

    this.setData({
      thumbnailBusy: true,
      thumbnailError: "",
      thumbnailResult: null,
    });
    try {
      const thumbnail = await lx.extractVideoThumbnail(payload);
      this.setData({ thumbnailResult: thumbnail, thumbnailBusy: false });
    } catch (error) {
      const message = error?.message || "extractVideoThumbnail failed";
      this.setData({
        thumbnailError: message,
        thumbnailResult: null,
        thumbnailBusy: false,
      });
      lx.showToast({
        title: message,
        icon: "none",
      });
    }
  },

  previewVideoThumbnail: async function () {
    const path = this.data.thumbnailResult?.tempFilePath;
    if (!path) {
      lx.showToast({
        title: "No thumbnail to preview",
        icon: "none",
      });
      return;
    }

    await this._runPreviewSession({
      sources: [path],
      startIndex: 0,
      advance: "manual",
    });
  },

  onVideoCompressQualityInput: function (event) {
    const value = extractInputValue(event).trim().toLowerCase();
    this.setData({ videoCompressQuality: value });
  },

  onVideoCompressBitrateInput: function (event) {
    const value = extractInputValue(event);
    this.setData({ videoCompressBitrate: value });
  },

  onVideoCompressFpsInput: function (event) {
    const value = extractInputValue(event);
    this.setData({ videoCompressFps: value });
  },

  onVideoCompressResolutionInput: function (event) {
    const value = extractInputValue(event);
    this.setData({ videoCompressResolution: value });
  },

  compressSelectedVideo: async function () {
    if (this.data.videoCompressBusy) {
      return;
    }
    const sourcePath = this.data.thumbnailVideoPath;
    if (!sourcePath) {
      lx.showToast({
        title: "Please pick a video first",
        icon: "none",
      });
      return;
    }

    const quality = (this.data.videoCompressQuality || "").trim().toLowerCase();
    const payload = { path: sourcePath };
    if (quality) {
      payload.quality = quality;
    } else {
      const bitrate = parsePositiveInt(this.data.videoCompressBitrate);
      const fps = parsePositiveInt(this.data.videoCompressFps);
      const resolution = parseResolutionRatio(this.data.videoCompressResolution);
      if (typeof bitrate === "number") {
        payload.bitrate = bitrate;
      }
      if (typeof fps === "number") {
        payload.fps = fps;
      }
      if (typeof resolution === "number") {
        payload.resolution = resolution;
      }
    }

    this.setData({
      videoCompressBusy: true,
      videoCompressError: "",
      videoCompressResult: null,
      videoCompressProgress: 0,
    });

    const task = lx.compressVideo(payload);
    this._compressVideoTask = task;
    try {
      for await (const { progress } of task) {
        this.setData({ videoCompressProgress: progress });
      }
      const result = await task.wait();
      this.setData({
        videoCompressResult: result,
        videoCompressBusy: false,
        videoCompressProgress: null,
      });
    } catch (error) {
      const isAbort = error?.name === "AbortError" || error?.code === "E_ABORT";
      const message = error?.message || "compressVideo failed";
      this.setData({
        videoCompressError: isAbort ? "" : message,
        videoCompressResult: null,
        videoCompressBusy: false,
        videoCompressProgress: null,
      });
      lx.showToast({
        title: isAbort ? "Compression cancelled" : message,
        icon: "none",
      });
    } finally {
      this._compressVideoTask = null;
    }
  },

  cancelVideoCompress: function () {
    this._compressVideoTask?.cancel();
  },

  previewCompressedVideo: async function () {
    const path = this.data.videoCompressResult?.tempFilePath;
    if (!path) {
      lx.showToast({ title: "No compressed video to preview", icon: "none" });
      return;
    }
    const source = { path, type: "video" };
    const rotate = resolveRotateValue(this.data.previewRotateKey || "meta");
    const objectFit = resolveObjectFitValue(this.data.previewObjectFitKey || "default", "contain");
    if (typeof rotate === "number") source.rotate = rotate;
    if (objectFit) source.objectFit = objectFit;
    await this._runPreviewSession({
      sources: [source],
      startIndex: 0,
      advance: "manual",
    });
  },

  onCompressQualityInput: function (event) {
    const value = extractInputValue(event);
    this.setData({ compressQuality: value });
  },

  onCompressedWidthInput: function (event) {
    const value = extractInputValue(event);
    this.setData({ compressedWidth: value });
  },

  onCompressedHeightInput: function (event) {
    const value = extractInputValue(event);
    this.setData({ compressedHeight: value });
  },

  compressSelectedImage: async function () {
    if (this.data.compressing) {
      return;
    }
    const path = this.data.imageInfoResult?.path;
    if (!path) {
      lx.showToast({
        title: "Please select an image first",
        icon: "none",
      });
      return;
    }

    const quality = parseQualityInput(this.data.compressQuality);
    const compressedWidth = parsePositiveInt(this.data.compressedWidth);
    const compressedHeight = parsePositiveInt(this.data.compressedHeight);

    const payload = { path, quality };
    if (typeof compressedWidth === "number") {
      payload.compressedWidth = compressedWidth;
    }
    if (typeof compressedHeight === "number") {
      payload.compressedHeight = compressedHeight;
    }

    this.setData({
      compressing: true,
      compressError: "",
      compressResult: null,
    });

    try {
      const result = await lx.compressImage(payload);
      const resultPath = result.tempFilePath;
      const info = await lx.getImageInfo({ path: resultPath });
      const size = await this._getFileSize(resultPath);
      this.setData({
        compressResult: { path: resultPath, width: info.width, height: info.height, type: info.type, size },
      });
    } catch (error) {
      const message = error?.message || "compressImage failed";
      this.setData({
        compressError: message,
        compressResult: null,
      });
      lx.showToast({
        title: message,
        icon: "none",
      });
    } finally {
      this.setData({ compressing: false });
    }
  },

  previewCompressedImage: async function () {
    const path = this.data.compressResult?.path;
    if (!path) {
      lx.showToast({
        title: "No compressed image to preview",
        icon: "none",
      });
      return;
    }

    await this._runPreviewSession({
      sources: [path],
      startIndex: 0,
      advance: "manual",
    });
  },

  _getFileSize: async function (path) {
    try {
      const stat = await lx.getFileManager().stat({ path });
      return stat.size || 0;
    } catch (error) {
      console.warn("Failed to get file size:", error);
      return 0;
    }
  },

  _pickSingleMedia: async function (type) {
    try {
      const result = await lx.chooseMedia({
        count: 1,
        mediaType: [type === "video" ? "video" : "image"],
        sourceType: ["album", "camera"],
        camera: "back",
      });
      return result[0]?.tempFilePath || null;
    } catch (error) {
      console.error("[media-demo] pickSingleMedia failed:", error);
      lx.showToast({ title: error?.message || "chooseMedia failed", icon: "none" });
      return null;
    }
  },

  captureImageForAlbum: async function () {
    if (this.data.saveToAlbumBusy) return;
    this.setData({ saveToAlbumBusy: true });
    try {
      const result = await lx.chooseMedia({
        count: 1, mediaType: ["image"], sourceType: ["camera"], camera: "back",
      });
      await lx.saveImageToPhotosAlbum({ filePath: result[0].tempFilePath });
      lx.showToast({ title: "Image saved to album", icon: "success" });
    } catch (error) {
      lx.showToast({ title: error?.message || "Failed to save image", icon: "none" });
    } finally {
      this.setData({ saveToAlbumBusy: false });
    }
  },

  captureVideoForAlbum: async function () {
    if (this.data.saveToAlbumBusy) return;
    this.setData({ saveToAlbumBusy: true });
    try {
      const result = await lx.chooseMedia({
        count: 1, mediaType: ["video"], sourceType: ["camera"], camera: "back", maxDuration: 60,
      });
      await lx.saveVideoToPhotosAlbum({ filePath: result[0].tempFilePath });
      lx.showToast({ title: "Video saved to album", icon: "success" });
    } catch (error) {
      lx.showToast({ title: error?.message || "Failed to save video", icon: "none" });
    } finally {
      this.setData({ saveToAlbumBusy: false });
    }
  },
});
