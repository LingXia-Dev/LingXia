Page({
  data: {
    currentType: '',
    deviceInfo: null,
    screenInfo: null
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
  },

  // Get screen information
  getScreenInfo: async function () {
    try {
      const screenInfo = await lx.getScreenInfo();
      console.log("Screen info:", screenInfo);

      await this.setData({
        screenInfo: screenInfo
      });
    } catch (error) {
      console.error("Failed to get screen info:", error);
    }
  },

  // Trigger short vibration
  vibrateShort: function () {
    try {
      lx.vibrateShort();
      console.log('Triggered short vibration');
    } catch (error) {
      console.error('Failed to trigger short vibration:', error);
    }
  },

  // Trigger long vibration
  vibrateLong: function () {
    try {
      lx.vibrateLong();
      console.log('Triggered long vibration');
    } catch (error) {
      console.error('Failed to trigger long vibration:', error);
    }
  }
});
