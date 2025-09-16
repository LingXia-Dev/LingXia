const app = getApp();

Page({
  data: {
    currentType: '',
    pageStack: [],
    modalResult: null
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
  },

  // Show toast with custom parameters
  showToastWithParams: function (params) {
    lx.showToast({
      title: params.title || 'Hello Toast!',
      icon: params.icon || 'success',
      duration: params.duration || 2000,
      position: params.position || 'center',
      mask: params.mask || false
    });
  },

  hideToast: function () {
    lx.hideToast();
  },

  // Show modal with custom parameters
  showModalWithParams: async function (params) {
    try {
      const result = await lx.showModal({
        title: params.title !== undefined ? params.title : 'Alert',
        content: params.content || 'This is a modal dialog',
        show_cancel: params.showCancel !== undefined ? params.showCancel : true,
        cancel_text: params.cancelText || 'Cancel',
        confirm_text: params.confirmText || 'OK'
      });

      // Filter out content field from result
      const filteredResult = {
        confirm: result.confirm,
        cancel: result.cancel
      };

      // Update page data with filtered result
      await this.setData({
        modalResult: filteredResult
      });

      return result;
    } catch (error) {
      console.error('Modal error:', error);
      const errorResult = { error: error.message };

      // Update page data with error
      await this.setData({
        modalResult: errorResult
      });

      throw error;
    }
  },

  // Clear modal result
  clearModalResult: async function () {
    await this.setData({
      modalResult: null
    });
  },

  // NavigationBar API functions
  setNavigationBarTitle: function (options) {
    console.log('setNavigationBarTitle called with:', options);
    const result = lx.setNavigationBarTitle(options);
    console.log('setNavigationBarTitle result:', result);
    return result;
  },

  setNavigationBarColor: function (options) {
    console.log('setNavigationBarColor called with:', options);
    const result = lx.setNavigationBarColor(options);
    console.log('setNavigationBarColor result:', result);
    return result;
  },

  // TabBar API functions
  showTabBarRedDot: function (options) {
    console.log('showTabBarRedDot called with:', options);
    const result = lx.showTabBarRedDot(options);
    console.log('showTabBarRedDot result:', result);
    return result;
  },

  hideTabBarRedDot: function (options) {
    console.log('hideTabBarRedDot called with:', options);
    const result = lx.hideTabBarRedDot(options);
    console.log('hideTabBarRedDot result:', result);
    return result;
  },

  setTabBarBadge: function (options) {
    console.log('setTabBarBadge called with:', options);
    const result = lx.setTabBarBadge(options);
    console.log('setTabBarBadge result:', result);
    return result;
  },

  removeTabBarBadge: function (options) {
    console.log('removeTabBarBadge called with:', options);
    const result = lx.removeTabBarBadge(options);
    console.log('removeTabBarBadge result:', result);
    return result;
  },

  showTabBar: function () {
    console.log('showTabBar called');
    const result = lx.showTabBar();
    console.log('showTabBar result:', result);
    return result;
  },

  hideTabBar: function () {
    console.log('hideTabBar called');
    const result = lx.hideTabBar();
    console.log('hideTabBar result:', result);
    return result;
  },

  setTabBarStyle: function (options) {
    console.log('setTabBarStyle called with:', options);
    const result = lx.setTabBarStyle(options);
    console.log('setTabBarStyle result:', result);
    return result;
  },

  setTabBarItem: function (options) {
    console.log('setTabBarItem called with:', options);
    const result = lx.setTabBarItem(options);
    console.log('setTabBarItem result:', result);
    return result;
  }
});
