const app = getApp();

function startWifiConnectedListener(page) {
  if (page.data.wifiListenerEnabled) {
    return;
  }

  const handler = (payload) => {
    const event = {
      id: `${Date.now()}-${Math.random().toString(16).slice(2)}`,
      time: new Date().toLocaleTimeString(),
      ...payload,
    };

    const nextEvents = [event, ...page.data.wifiConnectedEvents].slice(0, 5);
    page.setData({ wifiConnectedEvents: nextEvents });
  };

  page._wifiConnectedHandler = handler;
  try {
    lx.onWifiConnected(handler);
    page.setData({ wifiListenerEnabled: true });
  } catch (error) {
    console.error("Failed to register WiFi listener:", error);
    page._wifiConnectedHandler = null;
  }
}

function stopWifiConnectedListener(page) {
  if (!page.data.wifiListenerEnabled) {
    return;
  }
  try {
    lx.offWifiConnected(page._wifiConnectedHandler);
  } catch (error) {
    console.error("Failed to unregister WiFi listener:", error);
  } finally {
    page._wifiConnectedHandler = null;
    page.setData({ wifiListenerEnabled: false });
  }
}

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
    stopWifiConnectedListener(this);
  },

  onUnload: function () {
    console.log("WiFi page onUnload");
    // Cleanup: stop listener when page is destroyed
    stopWifiConnectedListener(this);
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
      stopWifiConnectedListener(this);
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
    startWifiConnectedListener(this);
  },

  offWifiConnected: function () {
    stopWifiConnectedListener(this);
  },

  clearWifiConnectedEvents: function () {
    this.setData({ wifiConnectedEvents: [] });
  },
});
