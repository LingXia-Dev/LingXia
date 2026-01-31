const app = getApp();

const NAV_TITLE_MAP = {
  navigation: "Navigation Demo",
  toast: "Toast Demo",
  actionsheet: "Action Sheet Demo",
  modal: "Modal Demo",
  navbar: "Navigation Bar Demo",
  tabbar: "Tab Bar Demo",
  popup: "Popup Demo",
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
    popupDemo: {
      message: "",
    },
  },

  onLoad: function (options) {
    console.log("UI page onLoad options:", options);

    // Pass querystring parameters to page via setData
    const type = options.type || "navigation";
    this.setData({
      currentType: type,
    });

    const title = NAV_TITLE_MAP[type] || "User Interface";
    this.setNavigationBarTitle({ title });

    // Update page stack immediately
    this.updatePageStack();
  },

  onShow: function () {
    console.log("UI page onShow");
    // Update page stack every time page shows
    this.updatePageStack();
  },

  // Update current page stack
  updatePageStack: function () {
    try {
      const pages = getCurrentPages();
      const pageStack = pages.map((page, index) => ({
        index: index,
        route: page.route || "unknown",
        options: page.options || {},
      }));

      this.setData({
        pageStack: pageStack,
      });
    } catch (error) {
      console.error("Failed to get current pages:", error);
    }
  },

  onHide: function () {
    console.log("UI page onHide");
  },

  demoNavigateTo: async function () {
    await lx.navigateTo({
      url: "pages/ui/index.tsx?type=navigation",
    });
  },

  demoNavigateBack: async function () {
    await lx.navigateBack({
      delta: 1,
    });
  },

  demoSwitchTab: async function () {
    await lx.switchTab({
      url: "pages/home/index.tsx",
    });
  },

  demoRedirectTo: async function () {
    await lx.redirectTo({
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

      if (
        typeof tapIndex !== "number" ||
        tapIndex < 0 ||
        tapIndex >= options.length
      ) {
        return;
      }

      lx.showToast({
        title: `Selected: ${options[tapIndex]}`,
        icon: "success",
        duration: 2000,
      });
    } catch (error) {
      console.log("Action sheet dismissed or failed:", error);
    }
  },

  showPopupDemo: async function (config) {
    const query = `source=ui-page&time=${Date.now()}`;

    this.setData({
      "popupDemo.message": "",
    });

    try {
      const cfg = config || {};
      const clamp = (value, fallback) => {
        const num = Number(value);
        if (Number.isFinite(num)) {
          return Math.min(1, Math.max(0.1, num));
        }
        return fallback;
      };

      const normalizedPosition =
        typeof cfg.position === "string"
          ? cfg.position.trim().toLowerCase()
          : "";
      const allowedPositions = new Set(["bottom", "center", "left", "right"]);
      const position = allowedPositions.has(normalizedPosition)
        ? normalizedPosition
        : "bottom";

      const fallbackWidth =
        position === "left" || position === "right" ? 0.72 : 0.9;
      const fallbackHeight =
        position === "left" || position === "right" ? 0.85 : 0.6;
      const widthRatio = clamp(cfg.widthRatio, fallbackWidth);
      const heightRatio = clamp(cfg.heightRatio, fallbackHeight);

      const popup = await lx.showPopup({
        url: `pages/popup/index.tsx?${query}`,
        position,
        widthRatio,
        heightRatio,
      });

      const handler = (payload) => {
        console.log("popupMessage received:", payload);

        const message =
          payload && typeof payload === "object"
            ? (payload.message ?? JSON.stringify(payload))
            : payload;

        const readable = typeof message === "string" ? message : "";

        this.setData({
          "popupDemo.message": readable,
        });

        popup.eventEmitter.off("popupMessage", handler);
      };

      popup.eventEmitter.on("popupMessage", handler);
    } catch (error) {
      console.error("showPopup failed:", error);
      this.setData({
        "popupDemo.message": `Failed: ${error.message}`,
      });
      lx.showToast({
        title: `showPopup failed: ${error.message}`,
        icon: "none",
      });
    }
  },

  // Show modal with custom parameters
  showModalWithParams: async function (params) {
    try {
      const result = await lx.showModal({
        title: params.title !== undefined ? params.title : "Alert",
        content: params.content || "This is a modal dialog",
        showCancel: params.showCancel !== undefined ? params.showCancel : true,
        cancelText: params.cancelText || "Cancel",
        confirmText: params.confirmText || "OK",
      });

      // Filter out content field from result
      const filteredResult = {
        confirm: result.confirm,
        cancel: result.cancel,
      };

      // Update page data with filtered result
      this.setData({
        modalResult: filteredResult,
      });

      return result;
    } catch (error) {
      console.error("Modal error:", error);
      const errorResult = { error: error.message };

      // Update page data with error
      this.setData({
        modalResult: errorResult,
      });

      throw error;
    }
  },

  // Clear modal result
  clearModalResult: function () {
    this.setData({
      modalResult: null,
    });
  },

  // NavigationBar API functions
  setNavigationBarTitle: function (options) {
    return lx.setNavigationBarTitle(options);
  },

  setNavigationBarColor: function (options) {
    return lx.setNavigationBarColor(options);
  },

  // TabBar API functions
  showTabBarRedDot: function (options) {
    return lx.showTabBarRedDot(options);
  },

  hideTabBarRedDot: function (options) {
    return lx.hideTabBarRedDot(options);
  },

  setTabBarBadge: function (options) {
    return lx.setTabBarBadge(options);
  },

  removeTabBarBadge: function (options) {
    return lx.removeTabBarBadge(options);
  },

  showTabBar: function () {
    return lx.showTabBar();
  },

  hideTabBar: function () {
    return lx.hideTabBar();
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
