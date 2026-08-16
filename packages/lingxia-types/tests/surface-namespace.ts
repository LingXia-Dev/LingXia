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

// An ordered preference must not be defeated by an option that applies to only
// one candidate — this is the one portable call the feature exists for.
async function preferenceKeepsPerPlacementOptions(): Promise<"window" | "float"> {
  const surface = await lx.surface.openPage("editor", {
    as: ["window", "float"],
    chrome: "full",
    position: "bottom",
    size: { width: "100%" },
  });
  return surface.realized;
}

// A float still takes a percentage size; the window/overlay split must not
// collapse it to `number`.
async function floatTakesPercentageSize(): Promise<void> {
  await lx.surface.openPage("feedback", {
    as: "float",
    size: { width: "100%", height: "80%" },
  });
}

// Instance keys and placement overrides are shell composition.
// @ts-expect-error lx.surface.openDeclared consumes the declaration as authored
lx.surface.openDeclared("terminal", { key: "project-a" });
const shellDeclared: Promise<unknown> = lx.shell.openDeclared("terminal", {
  key: "project-a",
  as: "main",
});

// A builtin page's lifetime belongs to the shell.
async function builtinReportsIdentityOnly(): Promise<string> {
  const settings = await lx.shell.openBuiltin("settings");
  // @ts-expect-error the shell owns a builtin page's visibility
  settings.show;
  return settings.id;
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
const fullChromeOffered: boolean = lx.supports({ capability: "surface", value: "window", chrome: "full" });
// @ts-expect-error 'frameless' is not a window chrome
lx.supports({ capability: "surface", value: "window", chrome: "frameless" });
// @ts-expect-error chrome only qualifies a window
lx.supports({ capability: "surface", value: "float", chrome: "full" });

// Capability answers are not surface members; `lx.supports` owns them.
const windowOffered: boolean = lx.supports({ capability: "surface", value: "window" });
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
  typeof preferenceKeepsPerPlacementOptions,
  typeof floatTakesPercentageSize,
  typeof builtinReportsIdentityOnly,
  typeof shellDeclared,
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
