Page({
  data: {
    currentType: "",
    deviceInfo: null,
    screenInfo: null,
    networkInfo: null,
    networkChange: null,
    networkListening: false,
    orientationListening: false,
    deviceOrientationValue: "",
    orientationEvents: [],
    orientationLock: "",
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
    this.stopDeviceOrientationListen();
  },

  onUnload: function () {
    this.stopNetworkChangeListen();
    this.stopDeviceOrientationListen();
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
    const rawPhoneNumber = typeof options?.phoneNumber === "string" ? options.phoneNumber : "";
    const normalizedPhoneNumber = rawPhoneNumber.replace(/^tel:/i, "").trim();
    if (!normalizedPhoneNumber) {
      return;
    }

    try {
      await lx.makePhoneCall({ phoneNumber: normalizedPhoneNumber });
      console.log("Making phone call to:", normalizedPhoneNumber);
    } catch (error) {
      console.error("Failed to make phone call:", error);
      lx.showToast({ title: error.message, icon: "none" });
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

  appendOrientationEvent: function (value) {
    const orientationValue = value === "portrait" || value === "landscape" ? value : "unknown";
    const timestamp = new Date().toISOString();
    const eventText = `${timestamp}  ${orientationValue}`;
    const currentEvents = Array.isArray(this.data.orientationEvents) ? this.data.orientationEvents : [];
    const nextEvents = [eventText, ...currentEvents].slice(0, 20);

    this.setData({
      orientationEvents: nextEvents,
      deviceOrientationValue: orientationValue,
    });
  },

  setOrientationPortrait: async function () {
    try {
      await lx.setDeviceOrientation("portrait");
      this.setData({ orientationLock: "portrait" });
    } catch (error) {
      console.error("Failed to set portrait orientation:", error);
      lx.showToast({ title: error.message, icon: "none" });
    }
  },

  setOrientationLandscape: async function () {
    try {
      await lx.setDeviceOrientation("landscape");
      this.setData({ orientationLock: "landscape" });
    } catch (error) {
      console.error("Failed to set landscape orientation:", error);
      lx.showToast({ title: error.message, icon: "none" });
    }
  },

  startDeviceOrientationListen: function () {
    if (this._deviceOrientationHandler) {
      return;
    }

    const handler = (event) => {
      const value = typeof event === "string" ? event : event?.value;
      this.appendOrientationEvent(value);
    };

    this._deviceOrientationHandler = handler;
    try {
      lx.onDeviceOrientationChange(handler);
      this.setData({ orientationListening: true });
    } catch (error) {
      this._deviceOrientationHandler = null;
      console.error("Failed to start device orientation listener:", error);
      lx.showToast({ title: error.message, icon: "none" });
    }
  },

  stopDeviceOrientationListen: function () {
    if (!this._deviceOrientationHandler) {
      return;
    }

    try {
      lx.offDeviceOrientationChange(this._deviceOrientationHandler);
    } catch (error) {
      console.error("Failed to stop device orientation listener:", error);
    }

    this._deviceOrientationHandler = null;
    this.setData({ orientationListening: false });
  },

  clearOrientationEvents: function () {
    this.setData({
      orientationEvents: [],
    });
  },
});
