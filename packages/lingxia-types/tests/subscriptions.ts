import type { AppearancePreference, DeviceOrientation, Live, Lx, NetworkInfo, ResolvedAppearance } from "../src/index.js";

declare const lx: Lx;

const unsubscribeNetwork: () => void = lx.network.status.watch(() => {});
const unsubscribeWifi: () => void = lx.network.wifi.onConnected(() => {});
const unsubscribeOrientation: () => void = lx.device.orientation.watch(() => {});
const unsubscribeKeyDown: () => void = lx.device.onKeyDown(() => {});
const unsubscribeKeyUp: () => void = lx.device.onKeyUp(() => {});

// The update manager is a listener set; each subscribe call returns its own
// unsubscribe handle, same idiom as the rest of the surface.
const updates = lx.app.update;
const unsubscribeUpdateReady: () => void = updates.onReady(() => {});
const secondReady: () => void = updates.onReady(() => {});
const unsubscribeUpdateFailed: () => void = updates.onFailed(() => {});
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

// Every subscription takes `{ signal }`.
const lifetime = new AbortController();
lx.network.status.watch(() => {}, { signal: lifetime.signal });
lx.host.appearance.watch(() => {}, { signal: lifetime.signal });
lx.device.orientation.watch(() => {}, { signal: lifetime.signal });
// @ts-expect-error options carry only a signal
lx.network.status.watch(() => {}, { once: true });

// Every value that changes is one `Live<T>`: `get()` and `watch()` agree on the value type.
const appearance: Live<ResolvedAppearance> = lx.app.appearance;
const appearancePreference: Live<AppearancePreference> = lx.host.appearance;
const language: Live<string> = lx.app.language;
const orientation: Live<DeviceOrientation> = lx.device.orientation;
const orientationNow: DeviceOrientation = lx.device.orientation.get();
lx.device.orientation.watch((value: DeviceOrientation) => void value);
// @ts-expect-error a lock belongs to a page: `page.orientation.lock`
lx.device.orientation.lock('landscape');
// @ts-expect-error the watcher receives the value itself, not `{ value }`
lx.device.orientation.watch((event: { value: DeviceOrientation }) => void event);
// @ts-expect-error the reported value is read-only
lx.device.orientation.set('landscape');
const network: Live<NetworkInfo> = lx.network.status;
const networkNow: NetworkInfo = lx.network.status.get();
// @ts-expect-error status is read with get(), synchronously
lx.network.status().then(() => {});
// @ts-expect-error follow connectivity through status.watch
lx.network.watch(() => {});
export type NetworkGate = [typeof network, typeof networkNow];
export type LiveGate = [typeof appearance, typeof appearancePreference, typeof language, typeof orientation, typeof orientationNow];
