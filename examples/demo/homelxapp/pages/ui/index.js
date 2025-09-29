const app = getApp();

const SINGLE_COLUMN_ITEMS = [
  "Espresso",
  "Latte",
  "Cappuccino",
  "Flat White",
  "Matcha Latte",
];

const CASCADING_COLUMNS = {
  Asia: ["Beijing", "Shanghai", "Singapore"],
  Europe: ["London", "Berlin", "Paris"],
  America: ["New York", "San Francisco", "Austin"],
};

const CASCADING_FIRST_COLUMN = Object.keys(CASCADING_COLUMNS);

const TIME_HOURS = Array.from({ length: 24 }, (_, index) =>
  index.toString().padStart(2, "0"),
);
const TIME_MINUTES = Array.from({ length: 60 }, (_, index) =>
  index.toString().padStart(2, "0"),
);

const PICKER_OPTIONS = {
  single: { mode: "selector", items: SINGLE_COLUMN_ITEMS },
  cascading: {
    mode: "multiSelector",
    columns: [CASCADING_FIRST_COLUMN, CASCADING_COLUMNS],
  },
  time: { mode: "time" },
};

const formatIndexLabel = (indexes) => {
  if (!Array.isArray(indexes) || indexes.length === 0) {
    return "--";
  }
  return indexes.length === 1 ? `${indexes[0]}` : `[${indexes.join(", ")}]`;
};

const PICKER_LABEL_FORMATTERS = {
  single: ([first = 0] = []) => SINGLE_COLUMN_ITEMS[first] || "--",
  cascading: ([first = 0, second = 0] = []) => {
    const region = CASCADING_FIRST_COLUMN[first] || "--";
    const city = (CASCADING_COLUMNS[region] || [])[second] || "--";
    return `${region} · ${city}`;
  },
  time: ([hourIndex = 0, minuteIndex = 0] = []) => {
    const hour = TIME_HOURS[hourIndex] || "00";
    const minute = TIME_MINUTES[minuteIndex] || "00";
    return `${hour}:${minute}`;
  },
};

const formatPickerLabel = (variant, indexes) => {
  const formatter = PICKER_LABEL_FORMATTERS[variant];
  if (!formatter) {
    return "--";
  }
  return formatter(indexes);
};

const createPickerEntry = (label = "--", index = "--", status = "Ready") => ({
  label,
  index,
  status,
});

const ensurePickerDemo = (page) => {
  const state = (page.data && page.data.pickerDemo) || {};
  return {
    streamingKey: state.streamingKey || "",
    single: state.single || createPickerEntry(),
    cascading: state.cascading || createPickerEntry(),
    time: state.time || createPickerEntry(),
  };
};

Page({
  data: {
    currentType: "",
    pageStack: [],
    modalResult: null,
    toastIcon: "success",
    toastIconLabel: "Success",
    toastIconOptions: [
      { label: "Success", value: "success" },
      { label: "Error", value: "error" },
      { label: "Loading", value: "loading" },
      { label: "None", value: "none" },
    ],
    toastPosition: "center",
    toastPositionLabel: "Center",
    toastPositionOptions: [
      { label: "Top", value: "top" },
      { label: "Center", value: "center" },
      { label: "Bottom", value: "bottom" },
    ],
    pickerDemo: {
      streamingKey: "",
      single: createPickerEntry(),
      cascading: createPickerEntry(),
      time: createPickerEntry(),
    },
    popupDemo: {
      message: "",
    },
  },

  onLoad: async function (options) {
    console.log("UI page onLoad options:", options);

    // Pass querystring parameters to page via setData
    await this.setData({
      currentType: options.type || "navigation",
    });

    // Update page stack immediately
    await this.updatePageStack();
  },

  onShow: async function () {
    console.log("UI page onShow");
    // Update page stack every time page shows
    await this.updatePageStack();
  },

  // Update current page stack
  updatePageStack: async function () {
    try {
      const pages = getCurrentPages();
      const pageStack = pages.map((page, index) => ({
        index: index,
        route: page.route || "unknown",
        options: page.options || {},
      }));

      await this.setData({
        pageStack: pageStack,
      });
    } catch (error) {
      console.error("Failed to get current pages:", error);
    }
  },

  onHide: function () {
    console.log("UI page onHide");
  },

  demoNavigateTo: function () {
    lx.navigateTo({
      url: "pages/ui/index.tsx?type=navigation",
    });
  },

  demoNavigateBack: function () {
    lx.navigateBack({
      delta: 1,
    });
  },

  demoSwitchTab: function () {
    lx.switchTab({
      url: "pages/home/index.html",
    });
  },

  demoRedirectTo: function () {
    lx.redirectTo({
      url: "pages/ui/index.tsx?type=navigation",
    });
  },

  // Show toast with custom parameters
  showToastWithParams: function (params) {
    const icon = params.icon || this.data.toastIcon || "success";
    const position = params.position || this.data.toastPosition || "center";
    lx.showToast({
      title: params.title || "Hello Toast!",
      icon,
      duration: params.duration || 2000,
      position,
      mask: params.mask || false,
    });
  },

  // Choose toast icon via action sheet
  chooseToastIcon: function () {
    const options = this.data.toastIconOptions || [];
    if (!options.length) {
      return;
    }

    lx.showActionSheet({
      itemList: options.map((option) => option.label),
      itemColor: "#007AFF",
    })
      .then((result) => {
        if (
          typeof result.tapIndex !== "number" ||
          result.tapIndex < 0 ||
          result.tapIndex >= options.length
        ) {
          return null;
        }

        const selected = options[result.tapIndex];
        return this.setData({
          toastIcon: selected.value,
          toastIconLabel: selected.label,
        });
      })
      .catch((error) => {
        console.log("chooseToastIcon cancelled or failed:", error);
      });
  },

  // Choose toast position via action sheet
  chooseToastPosition: function () {
    const options = this.data.toastPositionOptions || [];
    if (!options.length) {
      return;
    }

    lx.showActionSheet({
      itemList: options.map((option) => option.label),
      itemColor: "#007AFF",
    })
      .then((result) => {
        if (
          typeof result.tapIndex !== "number" ||
          result.tapIndex < 0 ||
          result.tapIndex >= options.length
        ) {
          return null;
        }

        const selected = options[result.tapIndex];
        return this.setData({
          toastPosition: selected.value,
          toastPositionLabel: selected.label,
        });
      })
      .catch((error) => {
        console.log("chooseToastPosition cancelled or failed:", error);
      });
  },

  hideToast: function () {
    lx.hideToast();
  },

  // Demo action sheet with mixed language options
  showDemoActionSheet: async function () {
    const options = ["View Details", "查看日志", "Send Email", "删除"];
    try {
      const { tapIndex } = await lx.showActionSheet({
        itemList: options,
        itemColor: "#007AFF",
      });

      lx.showToast({
        title: `Selected: ${options[tapIndex]}`,
        icon: "success",
        duration: 2000,
      });
    } catch (error) {
      console.log("Action sheet dismissed or failed:", error);
    }
  },

  showPopupDemo: async function () {
    const query = `source=ui-page&time=${Date.now()}`;

    await this.setData({
      "popupDemo.message": "",
    });

    try {
      if (this.popupDemoEmitter) {
        this.popupDemoEmitter.off("popupMessage");
        this.popupDemoEmitter = null;
      }

      const popup = lx.showPopup({
        url: `pages/popup/index.tsx?${query}`,
        position: "bottom",
        widthRatio: 1,
        heightRatio: 0.6,
      });

      const handler = (payload) => {
        console.log("popupMessage received:", payload);

        const message =
          payload && typeof payload === "object"
            ? (payload.message ?? JSON.stringify(payload))
            : payload;

        const readable = typeof message === "string" ? message : "";

        // this.setData({
        //   "popupDemo.message": readable,
        // });

        // mark, rong has bug, it cause deadlock
        //popup.eventEmitter.off("popupMessage", handler);
        this.popupDemoEmitter = null;
      };

      popup.eventEmitter.on("popupMessage", handler);
      this.popupDemoEmitter = popup.eventEmitter;
    } catch (error) {
      console.error("showPopup failed:", error);
      await this.setData({
        "popupDemo.message": `Failed: ${error.message}`,
      });
      lx.showToast({
        title: `showPopup failed: ${error.message}`,
        icon: "none",
      });
    }
  },

  startShowPickerDemo: async function (params) {
    const variant = (params && params.variant) || "single";
    const options = PICKER_OPTIONS[variant];
    if (!options) {
      return;
    }

    const currentKey =
      (this.data &&
        this.data.pickerDemo &&
        this.data.pickerDemo.streamingKey) ||
      "";
    if (currentKey && currentKey !== variant) {
      lx.showToast({
        title: "Finish the active picker first.",
        icon: "none",
        duration: 2000,
      });
      return;
    }
    if (currentKey === variant) {
      return;
    }

    let pickerState = ensurePickerDemo(this);
    pickerState = {
      ...pickerState,
      streamingKey: variant,
      [variant]: createPickerEntry("--", "--", "Listening..."),
    };
    await this.setData({ pickerDemo: pickerState });

    try {
      for await (const event of lx.showPicker(options)) {
        const raw = event && event.index;
        const indexes = Array.isArray(raw)
          ? raw.map((value) => Number(value) || 0)
          : raw !== undefined
            ? [Number(raw) || 0]
            : [];

        const label = formatPickerLabel(variant, indexes);
        const indexLabel = formatIndexLabel(indexes);
        const status =
          event && event.confirmed
            ? "Confirmed"
            : event && event.cancelled
              ? "Cancelled"
              : "Selecting...";

        pickerState = {
          ...pickerState,
          [variant]: createPickerEntry(label, indexLabel, status),
        };
        await this.setData({ pickerDemo: pickerState });

        if (event && (event.confirmed || event.cancelled)) {
          break;
        }
      }
    } catch (error) {
      const message = error && error.message ? error.message : "Picker failed";
      lx.showToast({ title: message, icon: "error", duration: 2000 });
      const previous = pickerState[variant] || createPickerEntry();
      pickerState = {
        ...pickerState,
        [variant]: createPickerEntry(previous.label, previous.index, "Error"),
      };
      await this.setData({ pickerDemo: pickerState });
    } finally {
      pickerState = {
        ...pickerState,
        streamingKey: "",
      };
      await this.setData({ pickerDemo: pickerState });
    }
  },

  resetShowPickerDemo: async function (params) {
    const variant = params && params.variant;
    if (!variant || !PICKER_OPTIONS[variant]) {
      return;
    }

    const currentKey =
      (this.data &&
        this.data.pickerDemo &&
        this.data.pickerDemo.streamingKey) ||
      "";
    if (currentKey === variant) {
      return;
    }

    const current = ensurePickerDemo(this);
    await this.setData({
      pickerDemo: {
        ...current,
        [variant]: createPickerEntry(),
      },
    });
  },

  // Show modal with custom parameters
  showModalWithParams: async function (params) {
    try {
      const result = await lx.showModal({
        title: params.title !== undefined ? params.title : "Alert",
        content: params.content || "This is a modal dialog",
        show_cancel: params.showCancel !== undefined ? params.showCancel : true,
        cancel_text: params.cancelText || "Cancel",
        confirm_text: params.confirmText || "OK",
      });

      // Filter out content field from result
      const filteredResult = {
        confirm: result.confirm,
        cancel: result.cancel,
      };

      // Update page data with filtered result
      await this.setData({
        modalResult: filteredResult,
      });

      return result;
    } catch (error) {
      console.error("Modal error:", error);
      const errorResult = { error: error.message };

      // Update page data with error
      await this.setData({
        modalResult: errorResult,
      });

      throw error;
    }
  },

  // Clear modal result
  clearModalResult: async function () {
    await this.setData({
      modalResult: null,
    });
  },

  // NavigationBar API functions
  setNavigationBarTitle: function (options) {
    console.log("setNavigationBarTitle called with:", options);
    const result = lx.setNavigationBarTitle(options);
    console.log("setNavigationBarTitle result:", result);
    return result;
  },

  setNavigationBarColor: function (options) {
    console.log("setNavigationBarColor called with:", options);
    const result = lx.setNavigationBarColor(options);
    console.log("setNavigationBarColor result:", result);
    return result;
  },

  // TabBar API functions
  showTabBarRedDot: function (options) {
    console.log("showTabBarRedDot called with:", options);
    const result = lx.showTabBarRedDot(options);
    console.log("showTabBarRedDot result:", result);
    return result;
  },

  hideTabBarRedDot: function (options) {
    console.log("hideTabBarRedDot called with:", options);
    const result = lx.hideTabBarRedDot(options);
    console.log("hideTabBarRedDot result:", result);
    return result;
  },

  setTabBarBadge: function (options) {
    console.log("setTabBarBadge called with:", options);
    const result = lx.setTabBarBadge(options);
    console.log("setTabBarBadge result:", result);
    return result;
  },

  removeTabBarBadge: function (options) {
    console.log("removeTabBarBadge called with:", options);
    const result = lx.removeTabBarBadge(options);
    console.log("removeTabBarBadge result:", result);
    return result;
  },

  showTabBar: function () {
    console.log("showTabBar called");
    const result = lx.showTabBar();
    console.log("showTabBar result:", result);
    return result;
  },

  hideTabBar: function () {
    console.log("hideTabBar called");
    const result = lx.hideTabBar();
    console.log("hideTabBar result:", result);
    return result;
  },

  setTabBarStyle: function (options) {
    console.log("setTabBarStyle called with:", options);
    const result = lx.setTabBarStyle(options);
    console.log("setTabBarStyle result:", result);
    return result;
  },

  setTabBarItem: function (options) {
    console.log("setTabBarItem called with:", options);
    const result = lx.setTabBarItem(options);
    console.log("setTabBarItem result:", result);
    return result;
  },
});
