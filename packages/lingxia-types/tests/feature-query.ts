import type { Lx, LxFeature, PageEnvContext } from "../src/index.js";

declare const lx: Lx;
declare const feature: LxFeature;
const known: boolean = lx.supports(feature);
const window: boolean = lx.supports('surface.window');
const fullChrome: boolean = lx.supports('surface.window.fullChrome');
const notifications: boolean = lx.supports('app.notification');
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
export type FeatureQueryGate = [typeof known, typeof window, typeof fullChrome, typeof notifications, typeof control, typeof docked];
