const DEFAULT_MODE = "Pictures";

const SOURCE_OPTIONS = [
  { key: "album", label: "Album", request: ["album"] },
  { key: "camera", label: "Camera", request: ["camera"] },
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
};

function getModeCopy(mediaType) {
  if (mediaType === "video") {
    return {
      headerSubtitle: "choose/previewMedia",
      emptyHint: "Tap + to add a video.",
      previewHint: "Tap the clip to replay.",
      galleryHint: "Tap the clip to replay.",
      addLabel: "Add Video",
    };
  }
  return {
    headerSubtitle: "choose/previewMedia",
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
    return resolveModeKey(input.mode);
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
    emptyHint: copy.emptyHint,
    previewHint: copy.previewHint,
    galleryHint: copy.galleryHint,
    headerSubtitle: copy.headerSubtitle,
    addLabel: copy.addLabel,
  };
}

Page({
  data: createState(DEFAULT_MODE),

  onLoad: function (options) {
    this.switchMode(options?.type);
  },

  switchMode: function (params) {
    const modeKey = resolveModeKey(params);
    const state = createState(modeKey);
    this.setData(state);

    const title =
      state.mediaType === "video" ? "Record / Pick Video" : "Photos";
    if (typeof this.setNavigationBarTitle === "function") {
      this.setNavigationBarTitle({ title });
    } else if (typeof lx.setNavigationBarTitle === "function") {
      lx.setNavigationBarTitle({ title });
    }
  },

  openSourcePicker: async function () {
    const choice = await pickOption(SOURCE_OPTIONS, this.data.sourceKey);
    if (!choice) {
      return;
    }
    const sourceChanged = choice.key !== this.data.sourceKey;
    const isCamera = choice.key === "camera";
    const updates = {
      sourceKey: choice.key,
    };

    if (sourceChanged) {
      updates.selectedMedia = [];
    }

    // Auto-set count limit to 1 when camera is selected for photos
    if (this.data.mediaType === "image" && isCamera) {
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
      mediaType: this.data.mediaType, // "image" | "video"
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
});
