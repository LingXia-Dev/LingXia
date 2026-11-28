import type { AnySurface, AppSurface, DeclaredSurface, Lx, LxErrorReason, PageHandle, PageSurface, TabSurface } from "../src/index.js";

declare const lx: Lx;
declare const wide: boolean;

// A surface refusal's sub-reason is one `LxErrorReason`, compared on `e.reason`.
const placement: LxErrorReason = "placementUnsupported";
// @ts-expect-error the snake_case surface codes are gone
const oldCode: LxErrorReason = "unsupported_placement";
void placement;
void oldCode;

// Probe 1: an illegal placement names the legal ones, instead of demanding an
// unrelated `appId` from a home-lxapp-only branch.
// @ts-expect-error a page cannot dock as an aside; `as` offers 'float' | 'window'
void lx.surface.open({ page: "inspector" }, { as: "aside" });

// Probe 2: a conditionally built call type-checks with no cast and no untyped
// intermediate variable.
async function conditionalOpenTypeChecks(): Promise<string> {
  const surface = lx.surface.open({ page: "inspector" }, {
    as: wide ? "window" : "float",
    size: { width: 480 },
  });
  return surface.realized;
}

// Probe 3: a union of handles narrows on `kind`, with no runtime sniffing.
function narrowsByKind(surface: AnySurface): string {
  if (surface.kind === "page") {
    surface.send({ hello: true });
    return surface.realized;
  }
  if (surface.kind === "tab") {
    return surface.scope;
  }
  return surface.kind;
}

// Probe 4: a url open returns a handle that can close what it opened.
async function urlResultCloses(): Promise<void> {
  const tab = await lx.surface.open({ url: "https://example.com" });
  if (tab.scope === "tab") {
    await tab.close();
    await tab.activate();
  }
}

// An ordered preference must not be defeated by an option that applies to only
// one candidate — this is the one portable call the feature exists for.
async function preferenceKeepsPerPlacementOptions(): Promise<"window" | "float"> {
  const surface = lx.surface.open({ page: "editor" }, {
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
  await lx.surface.open({ page: "feedback" }, {
    as: "float",
    size: { width: "100%", height: "80%" },
  }).opened;
}

// Instance keys and placement overrides are typed for everyone and refused at
// run time unless the caller is the Control app.
// @ts-expect-error a declared surface has no float/window size
lx.surface.open({ declared: "terminal" }, { size: { width: 480 } });
const shellDeclared: Promise<unknown> = lx.surface.open({ declared: "terminal" }, {
  key: "project-a",
  as: "main",
});

// Shell-owned surfaces are looked up by identity, not by a handle id.
function shellLookups(): boolean {
  const app = lx.surface.get({ app: "com.example.agent" });
  const terminal = lx.surface.get({ declared: "terminal", key: "project-a" });
  // @ts-expect-error handles carry no runtime id
  app?.id;
  // A declaration that shows an lxapp answers with that app's handle.
  const chat = lx.surface.get({ declared: "chat" });
  const chatRealized: "main" | "aside" | undefined =
    chat?.kind === "app" ? chat.realized : undefined;
  return Boolean(app?.alive && terminal?.alive && chatRealized);
}

// An ordered preference degrades, and `realized` reports the outcome.
async function orderedPreference(): Promise<"window" | "float"> {
  const surface = lx.surface.open({ page: "inspector" }, { as: ["window", "float"] });
  return surface.realized;
}

// Identity replaces caller-side bookkeeping.
async function identityReplacesCaching(): Promise<boolean> {
  await lx.surface.open({ page: "inspector" }, { as: "float", key: "inspector" }).opened;
  const live = lx.surface.get({ key: "inspector" });
  return live?.alive ?? false;
}

// `chrome: 'full'` is a window option, and the capability query can be asked
// about it before the affordance is offered.
async function edgeToEdgeWindow(): Promise<"window" | "float"> {
  // The target is `lx.nav.go`'s, params included.
  const win = lx.surface.open({ page: "editor", params: { draft: "42", pinned: true } }, {
    as: "window",
    chrome: "full",
  });
  return win.realized;
}
const fullChromeOffered: boolean = lx.supports('surface.window.fullChrome');
// @ts-expect-error there is no frameless contract
lx.supports("surface.window.frameless");
// @ts-expect-error fullChrome only belongs to window
lx.supports("surface.float.fullChrome");

// Capability answers are not surface members; `lx.supports` owns them.
const windowOffered: boolean = lx.supports('surface.window');
// @ts-expect-error capability queries do not live on lx.surface
lx.surface.can;
// @ts-expect-error capability queries do not live on lx.surface
lx.surface.capabilities;

// Another lxapp opens on lx.surface; its target is lx.nav.go's AppTarget.
const openedApp: Promise<unknown> = lx.surface.open(
  { app: "com.example.agent", page: "chat", params: { thread: 7 }, channel: "draft" },
  { as: "aside" },
);
// @ts-expect-error the startup query is params now
lx.surface.open({ app: "com.example.agent", query: { thread: "7" } });
// @ts-expect-error browser pages open with lx.surface.open({ app: "app.lingxia.browser" })
lx.shell.openBuiltin;
// @ts-expect-error the lookup is lx.surface.get
lx.surface.getByKey;
// @ts-expect-error moved to lx.surface.open
lx.shell.openApp;
// @ts-expect-error a url opens as 'main' or 'aside'
lx.surface.open({ url: "https://example.com" }, { as: "tab" });

// The retired API is gone.
// @ts-expect-error openSurface no longer exists
lx.openSurface;
// @ts-expect-error onSurfaceContext is gone
lx.onSurfaceContext;
// @ts-expect-error the size class is per page: page.env.watch
lx.surface.watchContext;
// @ts-expect-error a banner action's target brings the lxapp forward
lx.surface.bringToFront;
lx.shell.activate();

export type SurfaceNamespaceGate = [
  typeof preferenceKeepsPerPlacementOptions,
  typeof floatTakesPercentageSize,
  typeof shellDeclared,
  typeof openedApp,
  typeof edgeToEdgeWindow,
  typeof fullChromeOffered,
  typeof conditionalOpenTypeChecks,
  typeof narrowsByKind,
  typeof urlResultCloses,
  typeof orderedPreference,
  typeof identityReplacesCaching,
  typeof windowOffered,
];

// `as` decides which options exist: a float anchors, a window is decorated.
lx.surface.open({ page: "settings" }, { as: "float", position: "center" });
lx.surface.open({ page: "settings" }, { as: "window", chrome: "full" });
// @ts-expect-error a window does not anchor
lx.surface.open({ page: "settings" }, { as: "window", position: "center" });
// @ts-expect-error a float carries no window decoration
lx.surface.open({ page: "settings" }, { as: "float", chrome: "full" });
// An ordered preference cannot know which it will be, so both stay open.
lx.surface.open({ page: "settings" }, { as: ["window", "float"], chrome: "full", position: "center" });

// Params keep their JSON type through the page URL, as on `lx.nav.go`;
// only a non-JSON value is a compile error.
async function queryTakesJson(): Promise<void> {
  await lx.surface.open({ page: "editor", params: { filter: { open: true } } }, { as: "float" }).opened;
  // @ts-expect-error a Date is not JSON
  await lx.surface.open({ page: "editor", params: { at: new Date() } }, { as: "float" }).opened;
}

export type SurfaceQueryGate = [typeof queryTakesJson];

// `lx.shell` resolves to the global `ShellApi`, not the exported alias, so a
// member declared on only one of the two is unreachable at the call site it
// exists for. `sidebarActions` was exactly that.
async function sidebarActionsAreReachableThroughLxShell(): Promise<void> {
  await lx.shell.sidebarActions.replace([]);
  await lx.shell.sidebarActions.update("id", { label: "Renamed" });
  await lx.shell.sidebarActions.remove("id");
  // @ts-expect-error replace([]) is the one way to clear
  lx.shell.sidebarActions.clear();
}

// `value` is the footer's changing fact; a header action has none, and that is a
// compile error rather than a runtime rejection.
function sidebarActionValueIsFooterOnly(): void {
  const onSelect = () => {};
  lx.shell.sidebarActions.replace([
    { id: "brand", placement: "footer", icon: "public/brand.svg", label: "Brand", value: "Acme", onSelect },
    { id: "agent", placement: "footer", icon: "public/agent.svg", label: "Agent", onSelect },
    { id: "downloads", placement: "header", icon: "public/dl.svg", label: "Downloads", onSelect },
  ]);
  lx.shell.sidebarActions.replace([
    // @ts-expect-error a header action cannot carry a value
    { id: "brand", placement: "header", icon: "public/brand.svg", label: "Brand", value: "Acme", onSelect },
  ]);
  const brand = Math.random() > 0.5 ? { name: "Acme" } : undefined;
  lx.shell.sidebarActions.update("brand", { value: brand?.name });
  lx.shell.sidebarActions.update("brand", { value: null });
}

export type ShellSidebarGate = [
  typeof sidebarActionsAreReachableThroughLxShell,
  typeof sidebarActionValueIsFooterOnly,
];

// A present key clears even when its value is undefined; only strings or a
// clear are accepted.
async function appDisplayPatchClears(): Promise<void> {
  const brand = Math.random() > 0.5 ? { name: "Acme", icon: "public/acme.png" } : undefined;
  await lx.shell.appDisplay.update({ name: brand?.name, icon: brand?.icon });
  await lx.shell.appDisplay.update({ name: null });
  // @ts-expect-error the patch takes no other keys
  await lx.shell.appDisplay.update({ title: "Acme" });
  // @ts-expect-error the icon is a path, not image data
  await lx.shell.appDisplay.update({ icon: new Uint8Array(1) });
}

export type AppDisplayGate = [typeof appDisplayPatchClears];

// A handle is not a promise and has no `then`: awaiting it gives back the
// handle itself, so waiting for the page is always `.opened`.
type Equal<A, B> = (<T>() => T extends A ? 1 : 2) extends (<T>() => T extends B ? 1 : 2) ? true : false;
type Handle = ReturnType<typeof lx.surface.open<{ page: "inspector" }>>;
const awaitingTheHandleWaitsForNothing: Equal<Awaited<Handle>, Handle> = true;
const navHandleIsNotThenable: Equal<Awaited<PageHandle>, PageHandle> = true;
// @ts-expect-error a page handle has no `then`
void lx.surface.open({ page: "inspector" }).then;
async function handleBeforeOpened(): Promise<void> {
  const surface = lx.surface.open({ page: "inspector" });
  surface.send("ready");
  void surface.hide();
  await surface.opened;
  const outcome = await surface.result;
  if (outcome.status === "canceled") void outcome.reason;
}

export type SurfaceHandleGate = [typeof handleBeforeOpened, typeof awaitingTheHandleWaitsForNothing, typeof navHandleIsNotThenable];

// One `open`: the target says what (lx.nav.go's shapes), the options where and how.
// A page's handle comes back at once; anything else is a promise.
const pageOpened: PageSurface = lx.surface.open({ page: "inspector" });
const urlOpened: Promise<TabSurface> = lx.surface.open({ url: "https://example.com" }, { as: "aside", edge: "right", key: "docs" });
const appOpened: Promise<AppSurface> = lx.surface.open({ app: "com.example.agent", version: "2.1.0" }, { as: "main" });
const declaredOpened: Promise<DeclaredSurface | AppSurface> = lx.surface.open({ declared: "terminal" }, { key: "project-a", as: "aside", edge: "left" });
// The same target object works for lx.nav.go and lx.surface.open.
const settings = { page: "settings" } as const;
void lx.nav.go(settings);
void lx.surface.open(settings, { as: "float" });

// A wrong option names the target's own options, one probe per target.
// @ts-expect-error 'edge' does not exist in type 'OpenPageOptions'
lx.surface.open({ page: "inspector" }, { edge: "left" });
// @ts-expect-error 'chrome' does not exist in type 'OpenUrlOptions'
lx.surface.open({ url: "https://example.com" }, { chrome: "full" });
// @ts-expect-error 'key' does not exist in type 'OpenAppOptions'
lx.surface.open({ app: "com.example.agent" }, { key: "agent" });
// @ts-expect-error 'position' does not exist in type 'OpenDeclaredOptions'
lx.surface.open({ declared: "terminal" }, { position: "center" });
// What to show belongs in the target, not the options.
// @ts-expect-error 'params' does not exist in type 'OpenPageOptions'
lx.surface.open({ page: "editor" }, { params: { draft: "42" } });
// @ts-expect-error a URL target carries no placement
lx.surface.open({ url: "https://example.com", as: "aside" });
// @ts-expect-error one target names one thing
lx.surface.open({ page: "inspector", url: "https://example.com" });
// @ts-expect-error a tab is switched to with lx.nav.go
lx.surface.open({ tab: "home" });

// Lookups are one get, by key, lxapp or declaration.
const byKey: PageSurface | TabSurface | undefined = lx.surface.get({ key: "docs" });
const byApp: AppSurface | undefined = lx.surface.get({ app: "com.example.agent" });
const byDeclared: DeclaredSurface | AppSurface | undefined = lx.surface.get({ declared: "terminal", key: "project-a" });
// @ts-expect-error an lxapp is found by its id alone
lx.surface.get({ app: "com.example.agent", key: "agent" });
// @ts-expect-error the old lookups are gone
lx.surface.declared;
// @ts-expect-error the old opens are gone
lx.surface.openPage;

export type SurfaceOpenGate = [
  typeof pageOpened, typeof urlOpened, typeof appOpened, typeof declaredOpened,
  typeof byKey, typeof byApp, typeof byDeclared,
];
