const app = getApp();

const NAV_TITLE_MAP = {
  navigation: "Navigation Demo",
  toast: "Toast Demo",
  actionsheet: "Action Sheet Demo",
  modal: "Modal Demo",
  navbar: "Navigation Bar Demo",
  tabbar: "Tab Bar Demo",
  surface: "Surface Demo",
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
    surfaceDemo: {
      message: "",
      supportsWindow: false,
      // True when a surface is currently open (visible or hidden). The hide/show
      // buttons are only meaningful while a surface exists; closing tears it
      // down and resets this flag.
      active: false,
      visible: false,
    },
  },

  _activeSurface: null,

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
    this._updatePageStack();
    this._updateSurfaceCapabilities();
  },

  onShow: function () {
    console.log("UI page onShow");
    // Update page stack every time page shows
    this._updatePageStack();
    this._updateSurfaceCapabilities();
  },

  _updateSurfaceCapabilities: function () {
    let supportsWindow = false;
    try {
      const info = lx.getDeviceInfo();
      const osName = String(info?.osName || "").toLowerCase();
      supportsWindow =
        osName.includes("mac") ||
        osName.includes("windows") ||
        osName.includes("linux");
    } catch (error) {
      console.warn("Failed to detect surface capabilities:", error);
    }
    this.setData({
      "surfaceDemo.supportsWindow": supportsWindow,
    });
  },

  // Update current page stack
  _updatePageStack: function () {
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
      page: "ui",
      query: { type: "navigation" },
    });
  },

  demoNavigateBack: async function () {
    await lx.navigateBack({
      delta: 1,
    });
  },

  demoSwitchTab: async function () {
    await lx.switchTab({
      page: "home",
    });
  },

  demoRedirectTo: async function () {
    await lx.redirectTo({
      page: "ui",
      query: { type: "navigation" },
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
  chooseToastIcon: async function () {
    try {
      const { tapIndex } = await lx.showActionSheet({
        itemList: this.data.toastIconOptions.map((option) => option.label),
        itemColor: "#007AFF",
      });
      const selected = this.data.toastIconOptions[tapIndex];
      this.setData({ toastIcon: selected.value, toastIconLabel: selected.label });
    } catch (error) {
      console.log("chooseToastIcon cancelled:", error);
    }
  },

  // Choose toast position via action sheet
  chooseToastPosition: async function () {
    try {
      const { tapIndex } = await lx.showActionSheet({
        itemList: this.data.toastPositionOptions.map((option) => option.label),
        itemColor: "#007AFF",
      });
      const selected = this.data.toastPositionOptions[tapIndex];
      this.setData({ toastPosition: selected.value, toastPositionLabel: selected.label });
    } catch (error) {
      console.log("chooseToastPosition cancelled:", error);
    }
  },

  hideToast: function () {
    lx.hideToast();
  },

  // Demo action sheet with mixed language options
  showDemoActionSheet: async function () {
    const items = ["View Details", "查看日志", "Send Email", "删除"];
    try {
      const { tapIndex } = await lx.showActionSheet({
        itemList: items,
        itemColor: "#007AFF",
      });
      lx.showToast({ title: `Selected: ${items[tapIndex]}`, icon: "success" });
    } catch (error) {
      console.log("Action sheet dismissed:", error);
    }
  },

  openSurfaceDemo: async function (config) {
    // Eagerly tear down any prior surface tracked in this demo so a stale
    // reference (e.g., a surface closed by the user via backdrop tap whose
    // onClose got lost) never prevents reopening. The platform also replaces
    // any active surface on a new present(), so this is belt-and-suspenders.
    if (this._activeSurface) {
      try {
        await this._activeSurface.close();
      } catch (error) {
        console.warn("[surfaceDemo] prior close failed:", error);
      }
      this._activeSurface = null;
    }

    this.setData({
      "surfaceDemo.message": "",
    });

    try {
      const cfg = config || {};
      const supportsWindow = this.data.surfaceDemo?.supportsWindow === true;
      const requestedKind = cfg.kind === "window" ? "window" : "overlay";
      const kind = requestedKind === "window" && supportsWindow ? "window" : "overlay";
      const isWindow = kind === "window";
      const options = {
        kind,
        path: "pages/surface/index",
        query: { source: "ui-page", kind, time: Date.now() },
        size: isWindow
          ? { width: cfg.width || 960, height: cfg.height || 720 }
          : {
              width: `${Math.round((cfg.widthRatio || 0.9) * 100)}%`,
              height: `${Math.round((cfg.heightRatio || 0.6) * 100)}%`,
            },
      };
      if (!isWindow) {
        options.position = cfg.position || "bottom";
      }
      const surface = await lx.surface.open(options);
      this._activeSurface = surface;

      this.setData({
        "surfaceDemo.message": `Opened ${kind}: ${surface.id}`,
        "surfaceDemo.active": true,
        "surfaceDemo.visible": true,
      });
      surface.onMessage((payload) => {
        // Messages from the surface page no longer auto-close it — that
        // would defeat the show/hide demo. We just log the payload and let
        // the user decide whether to hide, show again, or close.
        const message =
          payload && typeof payload === "object"
            ? payload.message || JSON.stringify(payload)
            : payload;
        const text = typeof message === "string" ? message : JSON.stringify(message);
        this.setData({
          "surfaceDemo.message": `Message: ${text}`,
        });
      });
      // Subscribe to surface-driven visibility transitions. Both opener-side
      // and page-side toggles flow through these events, so the parent UI
      // stays in sync even when the overlay hides itself via this.surface.hide().
      surface.onShow((event) => {
        this.setData({
          "surfaceDemo.visible": true,
          "surfaceDemo.message": `Shown ${event.id} (source=${event.source})`,
        });
      });
      surface.onHide((event) => {
        this.setData({
          "surfaceDemo.visible": false,
          "surfaceDemo.message": `Hidden ${event.id} (source=${event.source})`,
        });
      });
      surface.onClose((event) => {
        this._activeSurface = null;
        const currentMessage = this.data.surfaceDemo?.message || "";
        const closeMessage = `Closed ${event.id}: ${event.reason}`;
        const displayMessage = currentMessage.startsWith("Message:")
          ? `${currentMessage} (${closeMessage})`
          : closeMessage;
        this.setData({
          "surfaceDemo.message": displayMessage,
          "surfaceDemo.active": false,
          "surfaceDemo.visible": false,
        });
      });
    } catch (error) {
      console.error("lx.surface.open failed:", error);
      this.setData({
        "surfaceDemo.message": `Failed: ${error.message}`,
        "surfaceDemo.active": false,
        "surfaceDemo.visible": false,
      });
      lx.showToast({
        title: `open failed: ${error.message}`,
        icon: "none",
      });
    }
  },

  showActiveSurface: async function () {
    const surface = this._activeSurface;
    if (!surface) {
      return;
    }
    try {
      await surface.show();
      this.setData({
        "surfaceDemo.message": `Shown ${surface.id}`,
        "surfaceDemo.visible": true,
      });
    } catch (error) {
      console.warn("surface.show failed:", error);
      this.setData({
        "surfaceDemo.message": `Show failed: ${error.message}`,
      });
    }
  },

  hideActiveSurface: async function () {
    const surface = this._activeSurface;
    if (!surface) {
      return;
    }
    try {
      await surface.hide();
      this.setData({
        "surfaceDemo.message": `Hidden ${surface.id}`,
        "surfaceDemo.visible": false,
      });
    } catch (error) {
      console.warn("surface.hide failed:", error);
      this.setData({
        "surfaceDemo.message": `Hide failed: ${error.message}`,
      });
    }
  },

  closeActiveSurface: async function () {
    const surface = this._activeSurface;
    if (!surface) {
      return;
    }
    try {
      await surface.close();
    } catch (error) {
      console.warn("surface.close failed:", error);
    }
  },

  // Show modal with custom parameters
  showModalWithParams: async function (params) {
    const result = await lx.showModal({
      title: params.title ?? "Alert",
      content: params.content || "This is a modal dialog",
      showCancel: params.showCancel ?? true,
      cancelText: params.cancelText || "Cancel",
      confirmText: params.confirmText || "OK",
    });
    this.setData({ modalResult: result });
    return result;
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
