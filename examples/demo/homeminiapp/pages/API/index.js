Page({
  onLoad: function () {
    console.log("API page onLoad, app:", app);
    console.log("API page globalData:", globalData);
  },

  onShow: function () {
    console.log("API page onShow");
  },

  onHide: function () {
    console.log("API page onHide");
  },

  openMiniProgram: function (option) {
    console.log("getDeviceInfo:", JSON.stringify(lx.getDeviceInfo()));
    lx.navigateToMiniProgram(option);
  },
});
