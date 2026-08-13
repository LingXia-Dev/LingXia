import type { AnySurface, Lx } from "../src/index.js";

declare const lx: Lx;
declare const wide: boolean;

// Probe 1: an illegal placement names the legal ones, instead of demanding an
// unrelated `appId` from a home-lxapp-only branch.
// @ts-expect-error a page cannot dock as an aside; `as` offers 'float' | 'window'
void lx.surface.openPage("inspector", { as: "aside" });

// Probe 2: a conditionally built call type-checks with no cast and no untyped
// intermediate variable.
async function conditionalOpenTypeChecks(): Promise<string> {
  const surface = await lx.surface.openPage("inspector", {
    as: wide ? "window" : "float",
    size: { width: 480 },
  });
  return surface.realized;
}

// Probe 3: a union of handles narrows on `kind`, with no runtime sniffing.
function narrowsByKind(surface: AnySurface): string {
  if (surface.kind === "page") {
    surface.postMessage({ hello: true });
    return surface.realized;
  }
  if (surface.kind === "tab") {
    return surface.scope;
  }
  return surface.id;
}

// Probe 4: a url open returns a handle that can close what it opened.
async function urlResultCloses(): Promise<void> {
  const tab = await lx.surface.openUrl("https://example.com");
  await tab.close();
  await tab.activate();
}

// An ordered preference degrades, and `realized` reports the outcome.
async function orderedPreference(): Promise<"window" | "float"> {
  const surface = await lx.surface.openPage("inspector", { as: ["window", "float"] });
  return surface.realized;
}

// Identity replaces caller-side bookkeeping.
async function identityReplacesCaching(): Promise<boolean> {
  await lx.surface.openPage("inspector", { as: "float", key: "inspector" });
  const live = lx.surface.get("inspector");
  return live?.alive ?? false;
}

// `chrome: 'full'` is a window option, and the capability query can be asked
// about it before the affordance is offered.
async function edgeToEdgeWindow(): Promise<"window" | "float"> {
  const win = await lx.surface.openPage("/pages/editor/index", {
    as: "window",
    chrome: "full",
  });
  return win.realized;
}
const fullChromeOffered: boolean = lx.supports({ surface: "window", chrome: "full" });
// @ts-expect-error 'frameless' is not a window chrome
lx.supports({ surface: "window", chrome: "frameless" });
// @ts-expect-error chrome only qualifies a window
lx.supports({ surface: "float", chrome: "full" });

// Capability answers are not surface members; `lx.supports` owns them.
const windowOffered: boolean = lx.supports({ surface: "window" });
// @ts-expect-error capability queries do not live on lx.surface
lx.surface.can;
// @ts-expect-error capability queries do not live on lx.surface
lx.surface.capabilities;

// Privilege is the namespace: composition lives on lx.shell only.
// @ts-expect-error opening another lxapp is a shell operation
lx.surface.openApp;
// @ts-expect-error opening a builtin page is a shell operation
lx.surface.openBuiltin;

// The retired API is gone.
// @ts-expect-error openSurface no longer exists
lx.openSurface;
// @ts-expect-error onSurfaceContext moved to lx.surface.onContext
lx.onSurfaceContext;

const unsubscribeContext: () => void = lx.surface.onContext(() => {});

export type SurfaceNamespaceGate = [
  typeof edgeToEdgeWindow,
  typeof fullChromeOffered,
  typeof conditionalOpenTypeChecks,
  typeof narrowsByKind,
  typeof urlResultCloses,
  typeof orderedPreference,
  typeof identityReplacesCaching,
  typeof windowOffered,
  typeof unsubscribeContext,
];
