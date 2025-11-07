const app = getApp();

Page({
  data: {
    // Category expansion state
    expandedSections: {
      interface: false,
      device: false,
      system: false,
      navigation: false,
      media: false,
      document: false,
    },
  },

  onLoad: function (options) {
    console.log("Options in onLoad: ", options);
  },

  onShow: function (options) {
    console.log("API page onShow");
    console.log("App data:", app.globalData);
  },

  onHide: function () {
    console.log("API page onHide");
  },

  // Toggle category expansion state
  toggleSection: function (params) {
    const { section } = params;
    const currentState = this.data.expandedSections[section];

    this.setData({
      [`expandedSections.${section}`]: !currentState,
    });
  },

  // Navigate to UI API page with specific type parameter
  navigateToUIPage: function (params) {
    const { type = "navigation" } = params || {};
    lx.navigateTo({
      url: `pages/ui/index.tsx?type=${type}`,
    });
  },

  // Navigate to Device API page with specific type parameter
  navigateToDevicePage: function (params) {
    const { type = "device" } = params || {};
    lx.navigateTo({
      url: `pages/device/index.tsx?type=${type}`,
    });
  },

  // Navigate to System API page with specific type parameter
  navigateToSystemPage: function (params) {
    const { type = "appBaseInfo" } = params || {};
    lx.navigateTo({
      url: `pages/system/index.tsx?type=${type}`,
    });
  },

  // Navigate to Location API page
  navigateToLocationPage: function () {
    lx.navigateTo({
      url: `pages/location/index.tsx`,
    });
  },

  // Navigate to Media API page with specific type parameter
  navigateToMediaPage: function (params) {
    const { type = "Pictures" } = params || {};
    lx.navigateTo({
      url: `pages/media/index.tsx?type=${type}`,
    });
  },

  // Navigate to Document API page
  navigateToDocumentPage: function () {
    lx.navigateTo({
      url: `pages/document/index.tsx`,
    });
  },

  navigateToTestMiniApp: async function () {
    try {
      await lx.navigateToLxApp({
        appId: "testminiapp",
        path: "pages/home/index.html?a=100&X=bcd",
      });
    } catch (err) {
      console.error("navigateToLxApp failed", err);
    }
  },
});
