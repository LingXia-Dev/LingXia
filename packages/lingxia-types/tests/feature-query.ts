import type { AutostartApi, BannerApi, Lx, LxFeature, NotificationApi, PageEnvContext, TerminalSettingsApi, ConptyApi } from "../src/index.js";

declare const lx: Lx;
declare const feature: LxFeature;
const known: boolean = lx.supports(feature);
const window: boolean = lx.supports('surface.window');
const fullChrome: boolean = lx.supports('surface.window.fullChrome');
const notifications: boolean = lx.supports('host.notification');
const control: boolean = lx.app.info.role === 'control';
const docked = (env: PageEnvContext): boolean => env.canDock;
// @ts-expect-error unknown strings are runtime-compatible but statically checked
lx.supports('future.feature');
// @ts-expect-error old objects are no longer accepted
lx.supports({ capability: 'terminal' });
// @ts-expect-error layout availability is page.env.canDock, not a feature
lx.supports('surface.aside');
// @ts-expect-error control is identity, not a feature
lx.supports('control');
// @ts-expect-error cache access follows Control identity
lx.supports('app.cache');
// @ts-expect-error main is a baseline placement
lx.supports('surface.main');
// @ts-expect-error float is a baseline placement
lx.supports('surface.float');
// @ts-expect-error option key/value paths are not contracts
lx.supports('surface.window.chrome.full');
// @ts-expect-error no implicit coercion
lx.supports(42);

// A capability namespace is present on every host, guarded or not; without the
// capability its calls reject `E_NOT_SUPPORTED`, so no `?.` is needed.
if (lx.supports('host.autostart')) {
  const guarded: AutostartApi = lx.host.autostart;
  void guarded.enable();
}
const autostart: AutostartApi = lx.host.autostart;
const notification: NotificationApi = lx.host.notification;
const banner: BannerApi = lx.host.banner;
const terminalSettings: TerminalSettingsApi = lx.host.terminal.settings;
void lx.host.autostart.disable();
const windowsAlways: ConptyApi = lx.host.terminal.conpty;
if (lx.supports('host.terminal.conpty')) void lx.host.terminal.conpty.status();
export type FeatureQueryGate = [typeof known, typeof window, typeof fullChrome, typeof notifications, typeof control, typeof docked, typeof autostart, typeof notification, typeof banner, typeof terminalSettings, typeof windowsAlways];
