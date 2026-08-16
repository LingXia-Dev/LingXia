import type {
  Lx,
  LxCapabilityFlag,
  LxCapabilityQuery,
  LxSurfaceCapability,
} from "../src/index.js";

declare const lx: Lx;

const canWindow: boolean = lx.supports({ capability: "surface", value: "window" });
const canAside: boolean = lx.supports({ capability: "surface", value: "aside" });
const hasTerminal: boolean = lx.supports({ capability: "terminal" });
const hasAutostart: boolean = lx.supports({ capability: "autostart" });
const hasNotifications: boolean = lx.supports({ capability: "notifications" });
const hasBrowser: boolean = lx.supports({ capability: "browser" });
const hasProxy: boolean = lx.supports({ capability: "proxy" });
const hasSelfUpdate: boolean = lx.supports({ capability: "selfUpdate" });

// An unknown capability key is a compile error.
// @ts-expect-error there is no `teleport` capability
lx.supports({ capability: "teleport" });
// An invalid option value is a compile error.
// @ts-expect-error 'popover' is not a surface placement
lx.supports({ capability: "surface", value: "popover" });
// @ts-expect-error surface queries require a value
lx.supports({ capability: "surface" });
// @ts-expect-error boolean capabilities do not accept surface values
lx.supports({ capability: "terminal", value: "window" });
// @ts-expect-error capability queries reject unknown options
lx.supports({ capability: "browser", extra: true });

// The query is a closed union, so a stringly-typed catalog cannot creep back in.
// @ts-expect-error canIUse-style dotted strings are not accepted
lx.supports("surface.window");

declare const query: LxCapabilityQuery;
const dynamic: boolean = lx.supports(query);
const flag: LxCapabilityFlag = "terminal";
const placement: LxSurfaceCapability = "window";

export type CapabilityQueryGate = [
  typeof canWindow,
  typeof canAside,
  typeof hasTerminal,
  typeof hasAutostart,
  typeof hasNotifications,
  typeof hasBrowser,
  typeof hasProxy,
  typeof hasSelfUpdate,
  typeof dynamic,
  typeof flag,
  typeof placement,
];
