const app = getApp();

Page({
  onLoad: function(options) {
    console.log("Options in onLoad: ", options);
  },

  onShow: function(options) {
    console.log("API page onShow");
    console.log("App data:", app.globalData);
  },

  onHide: function() {
    console.log("API page onHide");
  },

  navigateToStreamPage: async function() {
    await lx.navigateTo({ page: "stream" });
  },

  navigateToChannelPage: async function() {
    await lx.navigateTo({ page: "channel" });
  },

  // Navigate to UI API page with specific type parameter
  navigateToUIPage: async function(params) {
    const { type = "navigation" } = params || {};
    await lx.navigateTo({
      page: "ui",
      query: { type },
    });
  },

  // Navigate to Device API page with specific type parameter
  navigateToDevicePage: async function(params) {
    const { type = "device" } = params || {};
    await lx.navigateTo({
      page: "device",
      query: { type },
    });
  },

  // Navigate to WiFi API page
  navigateToWifiPage: async function() {
    await lx.navigateTo({
      page: "wifi",
    });
  },

  // Navigate to System API page with specific type parameter
  navigateToSystemPage: async function(params) {
    const { type = "appBaseInfo" } = params || {};
    await lx.navigateTo({
      page: "system",
      query: { type },
    });
  },

  // Navigate to Location API page
  navigateToLocationPage: async function() {
    await lx.navigateTo({
      page: "location",
    });
  },

  // Navigate to Media API page with specific type parameter
  navigateToMediaPage: async function(params) {
    const { type = "Pictures" } = params || {};
    await lx.navigateTo({
      page: "media",
      query: { type },
    });
  },

  navigateToOpenFilePage: async function() {
    await lx.navigateTo({
      page: "file",
      query: { section: "openFile" },
    });
  },

  navigateToChooseFilePage: async function() {
    await lx.navigateTo({
      page: "file",
      query: { section: "chooseFile" },
    });
  },

  navigateToTestMiniApp: async function() {
    try {
      await lx.navigateToLxApp({
        appId: "lingxia-chat",
        page: "chat",
      });
    } catch (err) {
      console.error("navigateToLxApp failed", err);
      lx.showToast({ title: err.message, icon: "none" });
    }
  },

  navigateToCloudPage: async function(params) {
    const { type = "auth" } = params || {};
    await lx.navigateTo({
      page: "cloud",
      query: { type },
    });
  },

  openDeepSeek: async function() {
    const targets: Array<"self" | "external"> = ["self", "external"];

    try {
      const { tapIndex } = await lx.showActionSheet({
        itemList: targets,
        itemColor: "#007AFF",
      });
      if (tapIndex < 0 || tapIndex >= targets.length) {
        return;
      }
      await lx.openURL({
        url: "https://www.deepseek.com/",
        target: targets[tapIndex],
      });
    } catch (error) {
      if (error.message.toLowerCase().includes("cancel")) {
        return;
      }
      lx.showToast({ title: error.message, icon: "none" });
    }
  },

  exitApp: async function() {
    const result = await lx.showModal({
      title: "Exit App",
      content: "lx.app.exit() exits immediately. Close the host app now?",
      confirmText: "Exit",
      cancelText: "Cancel",
    });
    if (result.confirm) {
      lx.app.exit();
    }
  },

  // Navigate to PullDownRefresh API page
  navigateToPullDownRefreshPage: async function() {
    await lx.navigateTo({
      page: "pullToRefresh",
    });
  },
});
