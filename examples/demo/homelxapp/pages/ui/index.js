const app = getApp();

Page({
  data: {
    currentType: '',
    pageStack: []
  },

  onLoad: async function (options) {
    console.log("UI page onLoad options:", options);

    // Pass querystring parameters to page via setData
    await this.setData({
      currentType: options.type || 'navigation'
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
        route: page.route || 'unknown',
        options: page.options || {}
      }));

      await this.setData({
        pageStack: pageStack
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
      url: "pages/ui/index.tsx?type=navigation"
    });
  },

  demoNavigateBack: function () {
    lx.navigateBack({
      delta: 1
    });
  },

  demoSwitchTab: function () {
    lx.switchTab({
      url: "pages/home/index.html"
    });
  },

  demoRedirectTo: function () {
    lx.redirectTo({
      url: "pages/ui/index.tsx?type=navigation"
    });
  }
});
