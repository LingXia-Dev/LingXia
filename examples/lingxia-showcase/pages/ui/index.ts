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
    this._updatePageStack();
  },

  onShow: function () {
    console.log("UI page onShow");
    // Update page stack every time page shows
    this._updatePageStack();
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
      url: "pages/ui/index?type=navigation",
    });
  },

  demoNavigateBack: async function () {
    await lx.navigateBack({
      delta: 1,
    });
  },

  demoSwitchTab: async function () {
    await lx.switchTab({
      url: "pages/home/index",
    });
  },

  demoRedirectTo: async function () {
    await lx.redirectTo({
      url: "pages/ui/index?type=navigation",
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

  showPopupDemo: async function (config) {
    const query = `source=ui-page&time=${Date.now()}`;

    this.setData({
      "popupDemo.message": "",
    });

    try {
      const cfg = config || {};
      const popup = await lx.showPopup({
        url: `pages/popup/index?${query}`,
        position: cfg.position || "bottom",
        widthRatio: cfg.widthRatio || 0.9,
        heightRatio: cfg.heightRatio || 0.6,
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
