import {
  definePage,
  lx,
  type DeviceInfo,
  type NetworkInfo,
  type ScreenInfo,
  type PageLoadOptions,
} from '@lingxia/logic';
import { errorMessage } from "../../shared/lib/errors";
export default definePage({
  data: {
    currentType: "device",
    deviceInfo: null as DeviceInfo | null,
    screenInfo: null as ScreenInfo | null,
    networkInfo: null as NetworkInfo | null,
    networkChange: null as NetworkInfo | null,
    networkListening: false,
    orientationListening: false,
    deviceOrientationValue: "",
    orientationEvents: [] as string[],
    orientationLock: "",
  },
  setup(page) {
    // Unsubscribe handles live on the instance, not in `data`: they are not
    // serializable and the View never sees them.
    let offNetworkChange = null as (() => void) | null;
    let offDeviceOrientationChange = null as (() => void) | null;
    const helpers = {
      appendOrientationEvent(value: string) {
        const orientationValue = value === "portrait" || value === "landscape" ? value : "unknown";
        const timestamp = new Date().toISOString();
        const eventText = `${timestamp}  ${orientationValue}`;
        const currentEvents = Array.isArray(page.data.orientationEvents) ? page.data.orientationEvents : [];
        const nextEvents = [eventText, ...currentEvents].slice(0, 20);

        page.update(d => {
          d.orientationEvents = nextEvents;
          d.deviceOrientationValue = orientationValue;
        });
      },
    };
    // An action switching section loads the page again.
    async function load(options: PageLoadOptions = {}) {
      console.log("Device page onLoad options:", options);

      const { type: requested = "device" } = (options || {}) as { type?: string };
      const type = requested === "phone" ? "dial"
        : requested === "network" ? "networkType"
        : requested;
      if (type !== page.data.currentType) {
        page.update(d => { d.currentType = type; });
      }
    }

    const actions = {
      async selectSection({ type }: { type: string }) {
        actions.stopNetworkChangeListen();
        actions.stopDeviceOrientationListen();
        await load({ type });
      },
      // Get device information
      async getDeviceInfo() {
        try {
          const deviceInfo = lx.device.info;
          console.log("Device info:", deviceInfo);

          page.update(d => { d.deviceInfo = deviceInfo; });
        } catch (error) {
          console.error("Failed to get device info:", error);
        }
      },
      // Get screen information
      async getScreenInfo() {
        try {
          const screenInfo = lx.device.screen();
          console.log("Screen info:", screenInfo);

          page.update(d => { d.screenInfo = screenInfo; });
        } catch (error) {
          console.error("Failed to get screen info:", error);
        }
      },
      // Trigger short vibration
      async vibrateShort() {
        try {
          await lx.device.haptics();
          console.log("Triggered short vibration");
        } catch (error) {
          console.error("Failed to trigger short vibration:", error);
          lx.ui.toast(errorMessage(error, "Request failed"));
        }
      },
      // Trigger long vibration
      async vibrateLong() {
        try {
          await lx.device.haptics('long');
          console.log("Triggered long vibration");
        } catch (error) {
          console.error("Failed to trigger long vibration:", error);
          lx.ui.toast(errorMessage(error, "Request failed"));
        }
      },
      // Make a phone call
      async makePhoneCall(options?: { phoneNumber?: string }) {
        const rawPhoneNumber = typeof options?.phoneNumber === "string" ? options.phoneNumber : "";
        const normalizedPhoneNumber = rawPhoneNumber.replace(/^tel:/i, "").trim();
        if (!normalizedPhoneNumber) {
          return;
        }

        try {
          await lx.openExternal(`tel:${normalizedPhoneNumber}`);
          console.log("Making phone call to:", normalizedPhoneNumber);
        } catch (error) {
          console.error("Failed to make phone call:", error);
          lx.ui.toast(errorMessage(error, "Request failed"));
        }
      },
      getNetworkInfo() {
        const result = lx.network.status.get();
        page.update(d => { d.networkInfo = result; });
      },
      startNetworkChangeListen() {
        if (offNetworkChange) {
          return;
        }
        offNetworkChange = lx.network.status.watch((info) => {
          page.update(d => { d.networkChange = info; });
        });
        page.update(d => { d.networkListening = true; });
      },
      stopNetworkChangeListen() {
        if (!offNetworkChange) {
          return;
        }
        offNetworkChange();
        offNetworkChange = null;
        page.update(d => { d.networkListening = false; });
      },
      async setOrientationPortrait() {
        try {
          lx.device.orientation.lock("portrait");
          page.update(d => { d.orientationLock = lx.device.orientation.get(); });
        } catch (error) {
          console.error("Failed to set portrait orientation:", error);
          lx.ui.toast(errorMessage(error, "Request failed"));
        }
      },
      async setOrientationLandscape() {
        try {
          lx.device.orientation.lock("landscape");
          page.update(d => { d.orientationLock = lx.device.orientation.get(); });
        } catch (error) {
          console.error("Failed to set landscape orientation:", error);
          lx.ui.toast(errorMessage(error, "Request failed"));
        }
      },
      startDeviceOrientationListen() {
        if (offDeviceOrientationChange) {
          return;
        }

        try {
          offDeviceOrientationChange = lx.device.orientation.watch((value) => {
            helpers.appendOrientationEvent(value);
          });
          page.update(d => { d.orientationListening = true; });
        } catch (error) {
          console.error("Failed to start device orientation listener:", error);
          lx.ui.toast(errorMessage(error, "Request failed"));
        }
      },
      stopDeviceOrientationListen() {
        if (!offDeviceOrientationChange) {
          return;
        }

        try {
          offDeviceOrientationChange();
        } catch (error) {
          console.error("Failed to stop device orientation listener:", error);
        }

        offDeviceOrientationChange = null;
        page.update(d => { d.orientationListening = false; });
      },
      clearOrientationEvents() {
        page.update(d => { d.orientationEvents = []; });
      },
    };
    return {
      onLoad: load,
      onShow() {
        console.log("Device page onShow");
      },
      onHide() {
        console.log("Device page onHide");
        actions.stopNetworkChangeListen();
        actions.stopDeviceOrientationListen();
      },
      onUnload() {
        actions.stopNetworkChangeListen();
        actions.stopDeviceOrientationListen();
      },
      actions,
    };
  },
});
