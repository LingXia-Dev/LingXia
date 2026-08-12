import type { Lx } from "../src/index.js";

declare const lx: Lx;

const unsubscribeNetwork: () => void = lx.onNetworkChange(() => {});
const unsubscribeWifi: () => void = lx.onWifiConnected(() => {});
const unsubscribeOrientation: () => void = lx.onDeviceOrientationChange(() => {});
const unsubscribeKeyDown: () => void = lx.onKeyDown(() => {});
const unsubscribeKeyUp: () => void = lx.onKeyUp(() => {});

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
];
