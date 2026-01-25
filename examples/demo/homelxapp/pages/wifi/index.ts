const app = getApp();

Page({
  data: {
    wifiList: null,
    connectedWifi: null,
    wifiModuleEnabled: false,
    wifiListenerEnabled: false,
    wifiConnectedEvents: [],
  },

  onLoad: async function (options) {
    console.log("WiFi page onLoad options:", options);
    // Restore WiFi module state from app globalData
    const wifiModuleEnabled = !!(app.globalData && app.globalData.wifiModuleEnabled);
    if (wifiModuleEnabled) {
      this.setData({ wifiModuleEnabled: true });
    }
  },

  onShow: function () {
    console.log("WiFi page onShow");
    // Sync WiFi module state from app globalData
    const wifiModuleEnabled = !!(app.globalData && app.globalData.wifiModuleEnabled);
    if (wifiModuleEnabled !== this.data.wifiModuleEnabled) {
      this.setData({ wifiModuleEnabled: wifiModuleEnabled });
    }
  },

  onHide: function () {
    console.log("WiFi page onHide");
    // Stop WiFi listener to prevent events firing against hidden page
    this.stopWifiConnectedListener();
  },

  onUnload: function () {
    console.log("WiFi page onUnload");
    // Cleanup: stop listener when page is destroyed
    this.stopWifiConnectedListener();
  },

  // WiFi APIs
  startWifi: async function () {
    try {
      await lx.startWifi();
      console.log("WiFi module started");
      // Persist state to app globalData
      if (app.globalData) {
        app.globalData.wifiModuleEnabled = true;
      }
      this.setData({ wifiModuleEnabled: true });
    } catch (error) {
      console.error("Failed to start WiFi:", error);
      // Show toast with error message
      const errorMessage = error && error.message ? error.message : String(error);
      lx.showToast({
        title: errorMessage,
        icon: "none",
        duration: 2500,
      });
    }
  },

  stopWifi: async function () {
    try {
      await lx.stopWifi();
      console.log("WiFi module stopped");
      this.stopWifiConnectedListener();
      // Persist state to app globalData
      if (app.globalData) {
        app.globalData.wifiModuleEnabled = false;
      }
      this.setData({
        wifiModuleEnabled: false,
        wifiList: null,
        connectedWifi: null,
      });
    } catch (error) {
      console.error("Failed to stop WiFi:", error);
    }
  },

  getWifiList: async function () {
    try {
      if (!this.data.wifiModuleEnabled) {
        console.warn("WiFi module not initialized");
        return;
      }
      const wifiList = await lx.getWifiList();
      console.log("WiFi list:", wifiList);
      this.setData({ wifiList: wifiList });
    } catch (error) {
      console.error("Failed to get WiFi list:", error);
      // Check if error is "not supported" - handle error code 12005, Chinese text, and English text
      const errorMessage = error && (error.message || String(error));
      const errorCode = error && error.errCode;
      const isNotSupported =
        errorCode === 12005 ||
        (errorMessage &&
          (errorMessage.includes("12005") ||
            errorMessage.includes("不支持") ||
            /not\s*supported/i.test(errorMessage)));
      if (isNotSupported) {
        lx.showToast({
          title: "WiFi scanning not supported on this platform",
          icon: "none",
          duration: 2000,
        });
      }
    }
  },

  getConnectedWifi: async function () {
    try {
      if (!this.data.wifiModuleEnabled) {
        console.warn("WiFi module not initialized");
        return;
      }
      const connectedWifi = await lx.getConnectedWifi();
      console.log("Connected WiFi:", connectedWifi);
      this.setData({ connectedWifi: connectedWifi });
    } catch (error) {
      console.error("Failed to get connected WiFi:", error);
    }
  },

  connectWifi: async function (ssidOrOptions, password) {
    try {
      if (!this.data.wifiModuleEnabled) {
        console.warn("WiFi module not initialized");
        return;
      }
      let options = {};
      if (ssidOrOptions && typeof ssidOrOptions === "object") {
        options = { ...ssidOrOptions };
      } else {
        options = { SSID: ssidOrOptions, password: password };
      }

      if (!options.SSID && options.ssid) {
        options.SSID = options.ssid;
      }

      const request = {
        SSID: options.SSID,
        password: options.password,
      };

      if (!request.SSID) {
        console.warn("connectWifi requires SSID");
        return;
      }

      await lx.connectWifi(request);
      console.log("WiFi connection requested:", request.SSID);
      console.log("Actual connection status will be reported via onWifiConnected event");
    } catch (error) {
      console.error("Failed to connect to WiFi:", error);
    }
  },

  onWifiConnected: function () {
    this.startWifiConnectedListener();
  },

  offWifiConnected: function () {
    this.stopWifiConnectedListener();
  },

  startWifiConnectedListener: function () {
    if (this.data.wifiListenerEnabled) {
      return;
    }
    if (typeof lx.onWifiConnected !== "function") {
      console.warn("onWifiConnected not available");
      return;
    }

    const handler = (payload) => {
      const value = payload && typeof payload === "object" ? payload : {};
      const ssid = value.SSID || value.ssid || "";
      const bssid = value.BSSID || value.bssid || "";
      const secure = typeof value.secure === "boolean" ? value.secure : undefined;
      const signalStrength =
        typeof value.signalStrength === "number" ? value.signalStrength : undefined;
      const frequency =
        typeof value.frequency === "number" ? value.frequency : undefined;
      const connected =
        typeof value.connected === "boolean" ? value.connected : undefined;
      const state = typeof value.state === "string" ? value.state : undefined;
      const event = {
        id: `${Date.now()}-${Math.random().toString(16).slice(2)}`,
        time: new Date().toLocaleTimeString(),
        ssid: String(ssid || ""),
        bssid: bssid ? String(bssid) : "",
        secure: secure,
        signalStrength: signalStrength,
        frequency: frequency,
        connected: connected,
        state: state,
      };

      const nextEvents = [event, ...(this.data.wifiConnectedEvents || [])].slice(0, 5);
      this.setData({ wifiConnectedEvents: nextEvents });
    };

    this._wifiConnectedHandler = handler;
    try {
      const result = lx.onWifiConnected(handler);
      if (result && typeof result.then === "function") {
        result.catch((error) => {
          console.error("onWifiConnected failed:", error);
        });
      }
      this.setData({ wifiListenerEnabled: true });
    } catch (error) {
      console.error("onWifiConnected failed:", error);
      this._wifiConnectedHandler = null;
    }
  },

  stopWifiConnectedListener: function () {
    if (!this.data.wifiListenerEnabled) {
      return;
    }
    if (typeof lx.offWifiConnected !== "function") {
      return;
    }

    try {
      const result = lx.offWifiConnected(this._wifiConnectedHandler);
      if (result && typeof result.then === "function") {
        result.catch((error) => {
          console.error("offWifiConnected failed:", error);
        });
      }
    } catch (error) {
      console.error("offWifiConnected failed:", error);
    } finally {
      this._wifiConnectedHandler = null;
      this.setData({ wifiListenerEnabled: false });
    }
  },

  clearWifiConnectedEvents: function () {
    this.setData({ wifiConnectedEvents: [] });
  },
});
