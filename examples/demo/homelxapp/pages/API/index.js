const app = getApp();

Page({
  onLoad: function () {},

  onShow: function () {
    console.log("API page onShow");
    console.log("App data:", app.globalData);
  },

  onHide: function () {
    console.log("API page onHide");
  },

  getDeviceInfo: async function () {
    const deviceInfo = lx.getDeviceInfo();

    await this.setData({
      deviceInfo: deviceInfo,
      showDeviceInfo: true,
    });
  },

  navigateToTestMiniApp: function () {
    lx.navigateToLxApp({
      appId: "testminiapp",
      path: "/pages/home/index",
    });
  },
});
