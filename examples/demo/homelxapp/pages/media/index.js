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

function extractInputValue(event) {
  if (!event) return "";
  if (typeof event === "string") return event;
  if (typeof event?.detail?.value === "string") return event.detail.value;
  if (typeof event?.target?.value === "string") return event.target.value;
  return "";
}

function clampQualityInput(value) {
  const parsed = parseInt(String(value ?? ""), 10);
  if (Number.isNaN(parsed)) return 80;
  return Math.min(100, Math.max(0, parsed));
}

function parsePositiveInt(value) {
  const parsed = parseInt(String(value ?? ""), 10);
  if (Number.isNaN(parsed) || parsed <= 0) {
    return undefined;
  }
  return parsed;
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
      countKey: "1",
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
  SaveToAlbum: {
    mediaType: "saveToAlbum",
    defaults: {},
  },
};

function getModeCopy(mediaType) {
  if (mediaType === "video") {
    return {
      headerSubtitle: "lx.chooseMedia / lx.previewMedia",
      emptyHint: "Tap + to add a video.",
      previewHint: "Tap Preview for fullscreen playback.",
      galleryHint: "Tap Preview for fullscreen playback.",
      addLabel: "Add Video",
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
    previewHint: "Tap a photo to preview.",
    galleryHint: "Tap a photo to preview.",
    addLabel: "Add Photo",
  };
}

function resolveModeKey(input) {
  if (typeof input === "string" && input.trim()) {
    const normalized = input.trim().toLowerCase();
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
  if (!Array.isArray(options) || !options.length) {
    return fallback || null;
  }
  if (!key) {
    return fallback || options[0] || null;
  }
  const matched = options.find((option) => option.key === key);
  return matched || fallback || options[0] || null;
}

function resolveMediaTypeTokens(input) {
  if (Array.isArray(input) && input.length) {
    return input;
  }
  if (typeof input === "string") {
    const normalized = input.toLowerCase();
    if (normalized === "video") {
      return ["video"];
    }
    if (normalized === "image") {
      return ["image"];
    }
  }
  return ["image", "video"];
}

function extractMediaSource(item, mediaType) {
  if (!item || typeof item !== "object") {
    return null;
  }
  const path = typeof item.path === "string" ? item.path : "";
  if (!path) {
    return null;
  }

  const rawType =
    typeof item.fileType === "string" ? item.fileType.toLowerCase() : "";
  const normalizedType =
    rawType === "video" ? "video" : rawType === "image" ? "image" : null;

  return {
    path,
    type: normalizedType || (mediaType === "video" ? "video" : "image"),
  };
}

function collectSources(result, mediaType) {
  if (!result) {
    return [];
  }
  const items = Array.isArray(result) ? result : [result];
  return items
    .map((item) => extractMediaSource(item, mediaType))
    .filter(Boolean);
}

async function pickOption(options, currentKey) {
  if (!Array.isArray(options) || !options.length) {
    return null;
  }

  const pickerConfig = {
    mode: "selector",
    items: options.map((option) => option.label),
  };
  const defaultIndex = options.findIndex((option) => option.key === currentKey);
  if (defaultIndex >= 0) {
    pickerConfig.defaultIndex = defaultIndex;
  }

  try {
    for await (const event of lx.showPicker(pickerConfig)) {
      if (event?.cancelled) {
        break;
      }
      const rawIndex = Array.isArray(event?.index)
        ? event?.index?.[0]
        : event?.index;
      const index = Number(rawIndex);
      if (Number.isNaN(index) || index < 0 || index >= options.length) {
        continue;
      }
      if (event?.confirmed) {
        return options[index];
      }
    }
  } catch (error) {
    console.error("[media-demo] showPicker failed:", error);
    lx.showToast({
      title: error?.message || "Operation failed",
      icon: "none",
    });
  }
  return null;
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
    compressMaxWidth: "",
    compressMaxHeight: "",
    compressing: false,
    compressResult: null,
    compressError: "",
  };
}

Page({
  data: createState(DEFAULT_MODE),

  onLoad: function (options) {
    this.switchMode(options?.type);
  },

  onHide: function () {
    this.setData({
      selectedMedia: [],
    });
  },

  switchMode: function (params) {
    const modeKey = resolveModeKey(params);
    const state = createState(modeKey);
    this.setData(state);

    const title =
      state.mediaType === "video"
        ? "Record / Pick Video"
        : state.mediaType === "scanCode"
          ? "Scan"
          : state.mediaType === "imageInfo"
            ? "Image Info"
            : state.mediaType === "saveToAlbum"
              ? "Save to Album"
              : "Photos";
    lx.setNavigationBarTitle({ title });
  },

  openSourcePicker: async function () {
    const choice = await pickOption(SOURCE_OPTIONS, this.data.sourceKey);
    if (!choice) {
      return;
    }
    const sourceChanged = choice.key !== this.data.sourceKey;
    const requestedSources = Array.isArray(choice.request)
      ? choice.request
      : ["album"];
    const isCameraOnly =
      requestedSources.includes("camera") &&
      !requestedSources.includes("album");
    const updates = {
      sourceKey: choice.key,
    };

    if (sourceChanged) {
      updates.selectedMedia = [];
    }

    // Auto-set count limit to 1 when camera is selected for photos
    if (this.data.mediaType === "image" && isCameraOnly) {
      updates.countKey = "1";
      updates.countLimit = 1;
    }

    this.setData(updates);
  },

  openCountPicker: async function () {
    if (!COUNT_OPTIONS.length) {
      return;
    }
    const choice = await pickOption(COUNT_OPTIONS, this.data.countKey);
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

  launchMediaDemo: async function () {
    if (this.data.mediaType === "scanCode") {
      this.startScan();
      return;
    }
    if (this.data.isRunning) return;

    const modeKey = this.data.modeKey || DEFAULT_MODE;
    const config = MODE_SETTINGS[modeKey] || MODE_SETTINGS[DEFAULT_MODE];
    const sourceOption = findOption(
      SOURCE_OPTIONS,
      this.data.sourceKey,
      SOURCE_OPTIONS[0],
    );

    // Build minimal request; latest chooseMedia returns a plain JS array
    const request = {
      mediaType: resolveMediaTypeTokens(this.data.mediaType),
      sourceType: sourceOption?.request || ["album"],
    };
    if (this.data.mediaType === "video") {
      request.count = 1;
      if (this.data.durationValue > 0)
        request.maxDuration = this.data.durationValue;
      if (this.data.cameraKey) request.camera = this.data.cameraKey;
    } else {
      // Respect count selection for images
      const n = parseInt(this.data.countLimit || this.data.countKey || "0", 10);
      if (n > 0) request.count = n;
    }

    this.setData({ isRunning: true, selectedMedia: [] });

    try {
      const results = await lx.chooseMedia(request);
      const mapped = results
        .map((it) => ({
          path: it?.tempFilePath || "",
          type: it?.fileType === "video" ? "video" : "image",
          isOriginal: !!it?.isOriginal,
        }))
        .filter((it) => it.path);
      // For video enforce single selection
      const finalList =
        this.data.mediaType === "video" ? mapped.slice(0, 1) : mapped;
      this.setData({ selectedMedia: finalList });
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
      const next = {
        scanBusy: false,
        scanResult: result?.scanResult || "",
        scanType: result?.scanType || "",
      };
      this.setData(next);
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

  previewSelectedMedia: async function (event) {
    const sources = this.data.selectedMedia || [];
    if (!sources.length) {
      lx.showToast({
        title: "Nothing to preview",
        icon: "none",
      });
      return;
    }

    const candidate = event?.item || event?.detail?.item || null;
    let target = candidate && typeof candidate === "object" ? candidate : null;

    if (!target) {
      const displayPath =
        event?.path ||
        event?.detail?.path ||
        event?.currentTarget?.dataset?.path ||
        "";
      target = sources.find((item) => item.path === displayPath);
    }

    if (!target) {
      target = sources[0];
    }

    const targetSource = {
      path: target.path,
      type: target.type,
    };

    try {
      await lx.previewMedia({
        sources: [targetSource],
        current: 0,
      });
    } catch (error) {
      console.error("[media-demo] previewMedia failed:", error);
      lx.showToast({
        title: error?.message || "Preview failed",
        icon: "none",
      });
    }
  },

  pickImageForInfo: async function () {
    if (this.data.imageInfoBusy) {
      return;
    }
    const picked = await this.pickSingleImage();
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
      // Get file size using Rong.stat
      let size = 0;
      try {
        const stat = await Rong.stat(picked);
        size = stat.size;
      } catch (statError) {
        console.warn("Failed to get file size:", statError);
      }
      this.setData({
        imageInfoResult: { ...info, size } || null,
        imageInfoBusy: false,
      });
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

  pickImageForCompress: async function () {
    if (this.data.compressing) {
      return;
    }
    const path = await this.pickSingleImage();
    if (!path) {
      return;
    }
    const quality = clampQualityInput(this.data.compressQuality);
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
      compressResultPath: "",
    });

    try {
      const result = await lx.compressImage(payload);
      this.setData({
        compressResultPath: result?.tempFilePath || "",
      });
    } catch (error) {
      const message = error?.message || "compressImage failed";
      this.setData({ compressError: message, compressResultPath: "" });
      lx.showToast({
        title: message,
        icon: "none",
      });
    } finally {
      this.setData({ compressing: false });
    }
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

    const quality = clampQualityInput(this.data.compressQuality);
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
      const resultPath = result?.tempFilePath || "";

      // Get complete image info for compressed image
      let compressResult = null;
      if (resultPath) {
        try {
          const info = await lx.getImageInfo({ path: resultPath });
          const stat = await Rong.stat(resultPath);
          compressResult = {
            path: resultPath,
            width: info.width,
            height: info.height,
            type: info.type,
            size: stat.size,
          };
        } catch (infoError) {
          console.warn("Failed to get compressed image info:", infoError);
          // Fallback to just path if getImageInfo fails
          compressResult = { path: resultPath };
        }
      }

      this.setData({
        compressResult: compressResult,
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

    try {
      await lx.previewMedia({
        sources: [{ path, type: "image" }],
        current: 0,
      });
    } catch (error) {
      console.error("[media-demo] previewCompressedImage failed:", error);
      lx.showToast({
        title: error?.message || "Preview failed",
        icon: "none",
      });
    }
  },

  pickSingleImage: async function () {
    try {
      const result = await lx.chooseMedia({
        count: 1,
        mediaType: ["image"],
        sourceType: ["album", "camera"],
        camera: "back",
      });
      const list = Array.isArray(result) ? result : result ? [result] : [];
      if (!list.length) {
        return null;
      }
      const first = list[0];
      const candidate = first?.tempFilePath || first?.path || first?.uri;
      return candidate || null;
    } catch (error) {
      console.error("[media-demo] pickSingleImage failed:", error);
      lx.showToast({
        title: error?.message || "chooseMedia failed",
        icon: "none",
      });
      return null;
    }
  },

  captureImageForAlbum: async function () {
    if (this.data.saveToAlbumBusy) {
      return;
    }
    this.setData({ saveToAlbumBusy: true });
    try {
      const result = await lx.chooseMedia({
        count: 1,
        mediaType: ["image"],
        sourceType: ["camera"],
        camera: "back",
      });
      const list = Array.isArray(result) ? result : result ? [result] : [];
      if (!list.length) {
        this.setData({ saveToAlbumBusy: false });
        return;
      }
      const first = list[0];
      const filePath = first?.tempFilePath || first?.path || first?.uri;
      if (!filePath) {
        throw new Error("No file path in result");
      }
      await lx.saveImageToPhotosAlbum({ filePath });
      this.setData({ saveToAlbumBusy: false });
      lx.showToast({
        title: "Image saved to album",
        icon: "success",
      });
    } catch (error) {
      const message = error?.message || "Failed to save image";
      this.setData({ saveToAlbumBusy: false });
      lx.showToast({
        title: message,
        icon: "none",
      });
    }
  },

  captureVideoForAlbum: async function () {
    if (this.data.saveToAlbumBusy) {
      return;
    }
    this.setData({ saveToAlbumBusy: true });
    try {
      const result = await lx.chooseMedia({
        count: 1,
        mediaType: ["video"],
        sourceType: ["camera"],
        camera: "back",
        maxDuration: 60,
      });
      const list = Array.isArray(result) ? result : result ? [result] : [];
      if (!list.length) {
        this.setData({ saveToAlbumBusy: false });
        return;
      }
      const first = list[0];
      const filePath = first?.tempFilePath || first?.path || first?.uri;
      if (!filePath) {
        throw new Error("No file path in result");
      }
      await lx.saveVideoToPhotosAlbum({ filePath });
      this.setData({ saveToAlbumBusy: false });
      lx.showToast({
        title: "Video saved to album",
        icon: "success",
      });
    } catch (error) {
      const message = error?.message || "Failed to save video";
      this.setData({ saveToAlbumBusy: false });
      lx.showToast({
        title: message,
        icon: "none",
      });
    }
  },
});
