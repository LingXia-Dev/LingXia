Page({
  data: {
    currentType: "",
    deviceInfo: null,
    screenInfo: null,
    networkInfo: null,
    networkChange: null,
    networkListening: false,
  },

  onLoad: async function (options) {
    console.log("Device page onLoad options:", options);

    // Pass querystring parameters to page via setData
    this.setData({
      currentType: options.type || "device",
    });
  },

  onShow: function () {
    console.log("Device page onShow");
  },

  onHide: function () {
    console.log("Device page onHide");
    this.stopNetworkChangeListen();
  },

  onUnload: function () {
    this.stopNetworkChangeListen();
  },

  // Get device information
  getDeviceInfo: async function () {
    try {
      const deviceInfo = lx.getDeviceInfo();
      console.log("Device info:", deviceInfo);

      this.setData({
        deviceInfo: deviceInfo,
      });
    } catch (error) {
      console.error("Failed to get device info:", error);
    }
  },

  // Get screen information
  getScreenInfo: async function () {
    try {
      const screenInfo = lx.getScreenInfo();
      console.log("Screen info:", screenInfo);

      this.setData({
        screenInfo: screenInfo,
      });
    } catch (error) {
      console.error("Failed to get screen info:", error);
    }
  },

  // Trigger short vibration
  vibrateShort: async function () {
    try {
      await lx.vibrateShort();
      console.log("Triggered short vibration");
    } catch (error) {
      console.error("Failed to trigger short vibration:", error);
      lx.showToast({ title: error.message, icon: "none" });
    }
  },

  // Trigger long vibration
  vibrateLong: async function () {
    try {
      await lx.vibrateLong();
      console.log("Triggered long vibration");
    } catch (error) {
      console.error("Failed to trigger long vibration:", error);
      lx.showToast({ title: error.message, icon: "none" });
    }
  },

  // Make a phone call
  makePhoneCall: async function (options) {
    try {
      await lx.makePhoneCall(options);
      console.log("Making phone call to:", options.phoneNumber);
    } catch (error) {
      console.error("Failed to make phone call:", error);
    }
  },

  getNetworkInfo: async function () {
    try {
      const result = await lx.getNetworkInfo();
      this.setData({
        networkInfo: result || null,
      });
    } catch (error) {
      console.error("Failed to get network info:", error);
    }
  },

  startNetworkChangeListen: function () {
    if (this._networkChangeHandler) {
      return;
    }
    const handler = (info) => {
      this.setData({
        networkChange: info || null,
      });
    };
    this._networkChangeHandler = handler;
    lx.onNetworkChange(handler);
    this.setData({ networkListening: true });
  },

  stopNetworkChangeListen: function () {
    if (!this._networkChangeHandler) {
      return;
    }
    lx.offNetworkChange(this._networkChangeHandler);
    this._networkChangeHandler = null;
    this.setData({ networkListening: false });
  },
});
