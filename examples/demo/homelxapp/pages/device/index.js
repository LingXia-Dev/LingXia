Page({
  data: {
    currentType: '',
    deviceInfo: null
  },

  onLoad: async function (options) {
    console.log("Device page onLoad options:", options);

    // Pass querystring parameters to page via setData
    await this.setData({
      currentType: options.type || 'device'
    });
  },

  onShow: function () {
    console.log("Device page onShow");
  },

  onHide: function () {
    console.log("Device page onHide");
  },

  // Get device information
  getDeviceInfo: async function () {
    try {
      const deviceInfo = lx.getDeviceInfo();
      console.log("Device info:", deviceInfo);

      await this.setData({
        deviceInfo: deviceInfo
      });
    } catch (error) {
      console.error("Failed to get device info:", error);
    }
  }
});
