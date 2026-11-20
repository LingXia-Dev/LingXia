import type { Lx } from "../src/index.js";

declare const lx: Lx;

const unsubscribeNetwork: () => void = lx.network.watch(() => {});
const unsubscribeWifi: () => void = lx.network.wifi.onConnected(() => {});
const unsubscribeOrientation: () => void = lx.device.orientation.watch(() => {});
const unsubscribeKeyDown: () => void = lx.device.onKeyDown(() => {});
const unsubscribeKeyUp: () => void = lx.device.onKeyUp(() => {});

// The update manager is a listener set; each subscribe call returns its own
// unsubscribe handle, same idiom as the rest of the surface.
const updates = lx.getUpdateManager();
const unsubscribeUpdateReady: () => void = updates.onUpdateReady(() => {});
const secondReady: () => void = updates.onUpdateReady(() => {});
const unsubscribeUpdateFailed: () => void = updates.onUpdateFailed(() => {});
void secondReady;

// No lx member can remove a listener it did not register.
// @ts-expect-error offNetworkChange no longer exists
lx.offNetworkChange;
// @ts-expect-error offWifiConnected no longer exists
lx.offWifiConnected;
// @ts-expect-error offDeviceOrientationChange no longer exists
lx.offDeviceOrientationChange;
// @ts-expect-error offKeyDown no longer exists
lx.offKeyDown;
// @ts-expect-error offKeyUp no longer exists
lx.offKeyUp;

export type SubscriptionGate = [
  typeof unsubscribeNetwork,
  typeof unsubscribeWifi,
  typeof unsubscribeOrientation,
  typeof unsubscribeKeyDown,
  typeof unsubscribeKeyUp,
  typeof unsubscribeUpdateReady,
  typeof unsubscribeUpdateFailed,
];
