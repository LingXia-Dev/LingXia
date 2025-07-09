Page({
  onLoad: function () {},

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
