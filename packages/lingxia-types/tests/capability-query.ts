import type { Lx, LxCapabilityQuery } from "../src/index.js";

declare const lx: Lx;

const canWindow: boolean = lx.supports({ surface: "window" });
const canAside: boolean = lx.supports({ surface: "aside" });
const hasTerminal: boolean = lx.supports({ terminal: true });
const hasAutostart: boolean = lx.supports({ autostart: true });
const hasNotifications: boolean = lx.supports({ notifications: true });
const hasBrowser: boolean = lx.supports({ browser: true });
const hasProxy: boolean = lx.supports({ proxy: true });
const hasSelfUpdate: boolean = lx.supports({ selfUpdate: true });
const hasNativeReview: boolean = lx.supports({ nativeFileReview: true });

// An unknown capability key is a compile error.
// @ts-expect-error there is no `teleport` capability
lx.supports({ teleport: true });
// An invalid option value is a compile error.
// @ts-expect-error 'popover' is not a surface placement
lx.supports({ surface: "popover" });
// The flags are declarations, not toggles.
// @ts-expect-error a capability flag is always `true`
lx.supports({ terminal: false });

// Note: a multi-key query cannot be a *type* error without giving every union
// member `?: never` siblings — the shape this batch exists to delete — so
// `lx.supports({ terminal: true, browser: true })` is rejected at runtime
// instead of silently answering whichever key came first.

// The query is a closed union, so a stringly-typed catalog cannot creep back in.
// @ts-expect-error canIUse-style dotted strings are not accepted
lx.supports("surface.window");

declare const query: LxCapabilityQuery;
const dynamic: boolean = lx.supports(query);

export type CapabilityQueryGate = [
  typeof canWindow,
  typeof canAside,
  typeof hasTerminal,
  typeof hasAutostart,
  typeof hasNotifications,
  typeof hasBrowser,
  typeof hasProxy,
  typeof hasSelfUpdate,
  typeof hasNativeReview,
  typeof dynamic,
];
