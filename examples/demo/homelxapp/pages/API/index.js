const app = getApp();

Page({
  data: {
    // Category expansion state
    expandedSections: {
      interface: false,
      device: false,
      navigation: false
    }
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
  toggleSection: async function (params) {
    const { section } = params;
    const currentState = this.data.expandedSections[section];

    await this.setData({
      [`expandedSections.${section}`]: !currentState
    });
  },

  // Navigate to UI API page with specific type parameter
  navigateToUIPage: function (params) {
    const { type = 'navigation' } = params || {};
    lx.navigateTo({
      url: `pages/ui/index.tsx?type=${type}`
    });
  },

  // Navigate to Device API page with specific type parameter
  navigateToDevicePage: function (params) {
    const { type = 'device' } = params || {};
    lx.navigateTo({
      url: `pages/device/index.tsx?type=${type}`
    });
  },

  navigateToTestMiniApp: function () {
    lx.navigateToLxApp({
      appId: "testminiapp",
      path: "pages/home/index.html?a=100&X=bcd",
    });
  },
});
