import type { ConnectWifiOptions } from "@lingxia/types";
import { showcaseApp } from "../../shared/lib/app";
import { errorMessage } from "../../shared/lib/errors";
const app = showcaseApp();

// The two module-level listener helpers take the page instance, so they need a
// name for the slice of it they touch.
interface WifiPage {
  data: { wifiListenerEnabled: boolean; wifiConnectedEvents: unknown[] };
  _offWifiConnected: (() => void) | null;
  setData(patch: Record<string, unknown>): void;
}

function startWifiConnectedListener(page: WifiPage) {
  if (page.data.wifiListenerEnabled) {
    return;
  }

  try {
    page._offWifiConnected = lx.onWifiConnected((payload) => {
      const event = {
        id: `${Date.now()}-${Math.random().toString(16).slice(2)}`,
        time: new Date().toLocaleTimeString(),
        ...payload,
      };

      const nextEvents = [event, ...page.data.wifiConnectedEvents].slice(0, 5);
      page.setData({ wifiConnectedEvents: nextEvents });
    });
    page.setData({ wifiListenerEnabled: true });
  } catch (error) {
    console.error("Failed to register WiFi listener:", error);
  }
}

function stopWifiConnectedListener(page: WifiPage) {
  if (!page.data.wifiListenerEnabled) {
    return;
  }
  try {
    page._offWifiConnected?.();
  } catch (error) {
    console.error("Failed to unregister WiFi listener:", error);
  } finally {
    page._offWifiConnected = null;
    page.setData({ wifiListenerEnabled: false });
  }
}

Page({
  data: {
    wifiList: null,
    connectedWifi: null,
    wifiModuleEnabled: false,
    wifiListenerEnabled: false,
    wifiConnectedEvents: [] as unknown[],
  },

  // The unsubscribe handle is not serializable state, so it lives on the
  // instance rather than in `data`.
  _offWifiConnected: null as (() => void) | null,

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
      lx.showToast({ title: errorMessage(error, "Wi-Fi request failed"), icon: "none" });
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
      lx.showToast({ title: errorMessage(error, "Wi-Fi request failed"), icon: "none" });
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

  connectWifi: async function (options: ConnectWifiOptions) {
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
