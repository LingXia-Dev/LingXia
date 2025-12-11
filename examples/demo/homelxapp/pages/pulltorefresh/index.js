Page({
  data: {
    refreshCount: 0,
    lastRefreshTime: null,
    isRefreshing: false,
  },

  onLoad: function (options) {
    console.log("PullDownRefresh page onLoad");
  },

  onShow: function () {
    console.log("PullDownRefresh page onShow");
  },

  onHide: function () {
    console.log("PullDownRefresh page onHide");
  },

  // Page lifecycle: Called when user pulls down to refresh
  onPullDownRefresh: function () {
    console.log("onPullDownRefresh triggered");
    const now = new Date();
    const timeString = now.toLocaleTimeString();

    this.setData({
      isRefreshing: true,
      refreshCount: this.data.refreshCount + 1,
      lastRefreshTime: timeString,
    });
  },

  startRefresh: function () {
    console.log("startPullDownRefresh called");
    lx.startPullDownRefresh();
  },

  stopRefresh: function () {
    console.log("stopPullDownRefresh called");
    this.setData({
      isRefreshing: false,
    });
    lx.stopPullDownRefresh();
  },
});
