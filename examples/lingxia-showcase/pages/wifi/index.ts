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
      lx.showToast({ title: error.message, icon: "none" });
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
      const wifiList = await lx.getWifiList();
      console.log("WiFi list:", wifiList);
      this.setData({ wifiList });
    } catch (error) {
      console.error("Failed to get WiFi list:", error);
      lx.showToast({ title: error.message, icon: "none" });
    }
  },

  getConnectedWifi: async function () {
    try {
      const connectedWifi = await lx.getConnectedWifi();
      console.log("Connected WiFi:", connectedWifi);
      this.setData({ connectedWifi });
    } catch (error) {
      console.error("Failed to get connected WiFi:", error);
    }
  },

  connectWifi: async function (options) {
    try {
      await lx.connectWifi(options);
      console.log("WiFi connection requested:", options?.SSID);
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

    const handler = (payload) => {
      const event = {
        id: `${Date.now()}-${Math.random().toString(16).slice(2)}`,
        time: new Date().toLocaleTimeString(),
        ...payload,
      };

      const nextEvents = [event, ...this.data.wifiConnectedEvents].slice(0, 5);
      this.setData({ wifiConnectedEvents: nextEvents });
    };

    this._wifiConnectedHandler = handler;
    try {
      lx.onWifiConnected(handler);
      this.setData({ wifiListenerEnabled: true });
    } catch (error) {
      console.error("Failed to register WiFi listener:", error);
      this._wifiConnectedHandler = null;
    }
  },

  stopWifiConnectedListener: function () {
    if (!this.data.wifiListenerEnabled) {
      return;
    }
    try {
      lx.offWifiConnected(this._wifiConnectedHandler);
    } catch (error) {
      console.error("Failed to unregister WiFi listener:", error);
    } finally {
      this._wifiConnectedHandler = null;
      this.setData({ wifiListenerEnabled: false });
    }
  },

  clearWifiConnectedEvents: function () {
    this.setData({ wifiConnectedEvents: [] });
  },
});
