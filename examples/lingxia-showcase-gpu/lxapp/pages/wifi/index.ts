import { definePage, lx, type ConnectWifiOptions, type WifiInfo } from '@lingxia/logic';
import { appState } from "../../shared/lib/app";
import { errorMessage } from "../../shared/lib/errors";

export default definePage({
  data: {
    statusText: "Ready",
    scanning: false,
    wifiList: null as WifiInfo[] | null,
    connectedWifi: null as WifiInfo | null,
    wifiModuleEnabled: false,
    wifiListenerEnabled: false,
    wifiConnectedEvents: [] as unknown[],
  },
  setup(page) {
    // The unsubscribe handle is not data, so it lives here, not in `data`.
    let _offWifiConnected = null as (() => void) | null;
    function startWifiConnectedListener() {
      if (page.data.wifiListenerEnabled) {
        return;
      }

      try {
        _offWifiConnected = lx.network.wifi.onConnected((payload) => {
          const event = {
            id: `${Date.now()}-${Math.random().toString(16).slice(2)}`,
            time: new Date().toLocaleTimeString(),
            ...payload,
          };

          const nextEvents = [event, ...page.data.wifiConnectedEvents].slice(0, 5);
          page.update(d => { d.wifiConnectedEvents = nextEvents; });
        });
        page.update(d => { d.wifiListenerEnabled = true; });
      } catch (error) {
        console.error("Failed to register WiFi listener:", error);
      }
    }

    function stopWifiConnectedListener() {
      if (!page.data.wifiListenerEnabled) {
        return;
      }
      try {
        _offWifiConnected?.();
      } catch (error) {
        console.error("Failed to unregister WiFi listener:", error);
      } finally {
        _offWifiConnected = null;
        page.update(d => { d.wifiListenerEnabled = false; });
      }
    }

    const actions = {
      async goBack() { await lx.nav.back(); },
      // WiFi APIs
      async startWifi() {
        try {
          await lx.network.wifi.start();
          console.log("WiFi module started");
          // Persist state to the shared app state
          appState.wifiModuleEnabled = true;
          page.update(d => {
            d.wifiModuleEnabled = true;
            d.statusText = "Module started";
          });
        } catch (error) {
          console.error("Failed to start WiFi:", error);
          page.update(d => { d.statusText = errorMessage(error, "Wi-Fi request failed"); });
          lx.ui.toast({ title: errorMessage(error, "Wi-Fi request failed"), icon: "none" });
        }
      },
      async stopWifi() {
        try {
          await lx.network.wifi.stop();
          console.log("WiFi module stopped");
          stopWifiConnectedListener();
          // Persist state to the shared app state
          appState.wifiModuleEnabled = false;
          page.update(d => {
            d.wifiModuleEnabled = false;
            d.wifiList = null;
            d.connectedWifi = null;
            d.statusText = "Module stopped";
          });
        } catch (error) {
          console.error("Failed to stop WiFi:", error);
          page.update(d => { d.statusText = errorMessage(error, "Wi-Fi request failed"); });
        }
      },
      async getWifiList() {
        if (page.data.scanning) return;
        page.update(d => {
          d.scanning = true;
          d.statusText = "Scanning nearby networks…";
        });
        try {
          const wifiList = await lx.network.wifi.list();
          console.log("WiFi list:", wifiList);
          page.update(d => {
            d.wifiList = wifiList;
            d.statusText = `Found ${wifiList.length} networks`;
          });
        } catch (error) {
          console.error("Failed to get WiFi list:", error);
          page.update(d => { d.statusText = errorMessage(error, "Wi-Fi scan failed"); });
          lx.ui.toast({ title: errorMessage(error, "Wi-Fi request failed"), icon: "none" });
        } finally {
          page.update(d => { d.scanning = false; });
        }
      },
      async getConnectedWifi() {
        try {
          const connectedWifi = await lx.network.wifi.current();
          console.log("Connected WiFi:", connectedWifi);
          page.update(d => {
            d.connectedWifi = connectedWifi;
            d.statusText = connectedWifi ? "Connected network read" : "No connected network";
          });
        } catch (error) {
          console.error("Failed to get connected WiFi:", error);
          page.update(d => { d.statusText = errorMessage(error, "Unable to read connected network"); });
        }
      },
      async connectWifi(options: ConnectWifiOptions) {
        try {
          await lx.network.wifi.connect(options);
          page.update(d => { d.statusText = "Connection requested"; });
          console.log("WiFi connection requested:", options?.ssid);
        } catch (error) {
          console.error("Failed to connect to WiFi:", error);
          page.update(d => { d.statusText = errorMessage(error, "Connection request failed"); });
        }
      },
      onWifiConnected() {
        startWifiConnectedListener();
      },
      offWifiConnected() {
        stopWifiConnectedListener();
      },
      clearWifiConnectedEvents() {
        page.update(d => { d.wifiConnectedEvents = []; });
      },
    };
    return {
      async onLoad(options) {
        console.log("WiFi page onLoad options:", options);
        // Restore WiFi module state from the shared app state
        const wifiModuleEnabled = appState.wifiModuleEnabled;
        if (wifiModuleEnabled) {
          page.update(d => {
            d.wifiModuleEnabled = true;
            d.statusText = "Module started";
          });
        }
      },
      onShow() {
        console.log("WiFi page onShow");
        // Sync WiFi module state from the shared app state
        const wifiModuleEnabled = appState.wifiModuleEnabled;
        if (wifiModuleEnabled !== page.data.wifiModuleEnabled) {
          page.update(d => { d.wifiModuleEnabled = wifiModuleEnabled; });
        }
      },
      onHide() {
        console.log("WiFi page onHide");
        // Stop WiFi listener to prevent events firing against hidden page
        stopWifiConnectedListener();
      },
      onUnload() {
        console.log("WiFi page onUnload");
        // Cleanup: stop listener when page is destroyed
        stopWifiConnectedListener();
      },
      actions,
    };
  },
});
