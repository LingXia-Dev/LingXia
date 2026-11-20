import { errorMessage } from "../../shared/lib/errors";
import type {
  ActionSheetResult,
  PageSurface,
  ToastHandle,
  ToastOptions,
  SurfaceEdge,
  SurfaceErrorCode,
  NavigationBarPatch,
  SurfaceFloatPosition,
  TabBarItemPatch,
} from "@lingxia/types";

/** What the modal demo shows: whether the dialog was confirmed. */
type ModalResult = { status: 'ok' } | { status: 'canceled' };
import { definePage } from '@lingxia/types/page';

type DemoSurfaceVerb = "float" | "window" | "lxapp" | "aside";

/** What the View sends when it asks for a demo surface. */
interface DemoSurfaceConfig {
  verb?: string;
  width?: number;
  height?: number;
  url?: string;
  appId?: string;
  page?: string;
  edge?: SurfaceEdge;
  position?: SurfaceFloatPosition;
  /** Window decoration. Ignored unless `verb` is `window`. */
  chrome?: "system" | "full";
}


// The type is erased at compile time. `SURFACE_ERROR_CODES` is a runtime export
// of the same package, and importing that would leave an external module in the
// concatenated Logic bundle — so the list is restated here.
const SURFACE_ERROR_CODES: readonly SurfaceErrorCode[] = [
  "unsupported_placement",
  "denied",
  "not_declared",
  "invalid_arg",
  "already_open_other_role",
  "role_locked",
  "closed",
  "capability_missing",
  "failed",
];

const DEMO_PAGE_SURFACE_KEY = "surface-demo-page";

const NAV_TITLE_MAP: Record<string, string> = {
  navigation: "Navigation Demo",
  toast: "Toast Demo",
  actionsheet: "Action Sheet Demo",
  modal: "Modal Demo",
  navbar: "Navigation Bar Demo",
  tabbar: "Tab Bar Demo",
  appearance: "Appearance Demo",
  surface: "Surface Demo",
};

function surfaceErrorObject(error: unknown): Record<string, unknown> | null {
  return typeof error === "object" && error !== null
    ? (error as Record<string, unknown>)
    : null;
}

function surfaceErrorCode(error: unknown): SurfaceErrorCode | null {
  const root = surfaceErrorObject(error);
  const data = surfaceErrorObject(root?.data);
  const code = data?.code;
  return typeof code === "string" && SURFACE_ERROR_CODES.includes(code as SurfaceErrorCode)
    ? (code as SurfaceErrorCode)
    : null;
}

function surfaceErrorMessage(error: unknown): string {
  const object = surfaceErrorObject(error);
  return typeof object?.message === "string"
    ? object.message
    : String(error || "unknown error");
}

const LAST_INSTANCE_TAG_KEY = "lifecycle:lastInstanceTag";
const MAX_EVENTS = 8;

// Module scope: evaluated once per app session, shared by every live
// instance of this route, and never reset by leaving the page. The value in
// `data` below is only a display mirror of this variable.
let moduleCounter = 0;

function newInstanceTag() {
  return Math.random().toString(16).slice(2, 6).toUpperCase();
}

export default definePage({
  data: {
    currentType: "navigation",
    instanceTag: "",
    previousInstanceTag: "",
    logicCounter: 0,
    moduleCounter: 0,
    events: [] as string[],
    modalResult: null as ModalResult | null,
    actionSheetResult: null as ActionSheetResult | null,
    toastIcon: "success" as NonNullable<ToastOptions["icon"]>,
    toastIconLabel: "Success",
    toastIconOptions: [
      { label: "Success", value: "success" },
      { label: "Error", value: "error" },
      { label: "Loading", value: "loading" },
      { label: "None", value: "none" },
    ] as { label: string; value: NonNullable<ToastOptions["icon"]> }[],
    toastPosition: "center" as NonNullable<ToastOptions["position"]>,
    toastPositionLabel: "Center",
    toastPositionOptions: [
      { label: "Top", value: "top" },
      { label: "Center", value: "center" },
      { label: "Bottom", value: "bottom" },
    ] as { label: string; value: NonNullable<ToastOptions["position"]> }[],
    surfaceDemo: {
      message: "",
      // True when a surface is currently open (visible or hidden). The hide/show
      // buttons are only meaningful while a surface exists; closing tears it
      // down and resets this flag.
      active: false,
      visible: false,
    },
    chromeError: "",
    appearance: { preference: "auto", resolved: "light" },
  },
  setup(thisPage) {
    // The toast the "hide" button dismisses.
    let lastToast: ToastHandle | undefined;
    let asideTabIndex = 0;
    const helpers = {
      // The log lives in `data`, so it only ever shows the current instance's
      // events — hide/show repeat within one instance, unload ends it.
      record(event: string) {
        const stamp = new Date().toLocaleTimeString();
        thisPage.update(d => { d.events = [...thisPage.data.events, `${stamp}  ${event}`].slice(-MAX_EVENTS); });
      },
      // Appearance is part of Page Chrome: the product picks light/dark and the
      // runtime projects the resolved scheme into every page as `color-scheme` plus
      // `data-theme` on <html>. Both halves are read from the runtime rather than
      // mirrored in page state.
      syncAppearance() {
        try {
          thisPage.update(d => {
            d.appearance = {
              preference: lx.host.appearance.getPreference() ?? "auto",
              resolved: lx.host.appearance.get(),
            };
          });
        } catch (error) {
          thisPage.update(d => { d.chromeError = `Appearance unavailable: ${surfaceErrorMessage(error)}`; });
        }
      },
      // A key is an identity, not a slot: every open mints a fresh surface, so
      // reusing a key without closing first leaves the old one on screen with
      // nothing pointing at it.
      async releaseDemoSurfaceKey(key: string) {
        const existing = lx.surface.get(key);
        if (!existing || !("close" in existing)) return;
        try {
          await existing.close();
        } catch (error) {
          console.warn("surface.close before reopen failed:", error);
        }
      },
      async openDemoSurface(
        verb: DemoSurfaceVerb,
        cfg: DemoSurfaceConfig,
        size: { width?: number; height?: number },
      ) {
        if (verb === "lxapp") {
          // Compose another lxapp into the aside slot — home-lxapp privilege, so
          // it lives on lx.shell rather than lx.surface. A live app is reused.
          return lx.surface.openApp("lingxia-chat", {
            as: "aside",
            edge: cfg.edge ?? "right",
          });
        }
        if (verb === "aside") {
          // Multi-tab demo: each click opens the next url as a tab in the one
          // browser aside (deduped by url). An aside is external content only.
          const demoUrls = [
            "https://www.deepseek.com/",
            "https://cn.bing.com/",
            "https://opensource.adobe.com/",
          ];
          const idx = (asideTabIndex || 0) % demoUrls.length;
          asideTabIndex = (asideTabIndex || 0) + 1;
          return lx.surface.openUrl(cfg.url ?? demoUrls[idx], {
            as: "aside",
            edge: cfg.edge ?? "right",
            size,
          });
        }
        await helpers.releaseDemoSurfaceKey(DEMO_PAGE_SURFACE_KEY);
        if (verb === "window") {
          // `full` runs the page to the window edge and keeps system min/max/close
          // plus a runtime drag strip. `system` is the standard title bar. The
          // demo lets the caller pick; default to full when the host offers it.
          const wantsFull = cfg.chrome !== "system"
            && lx.supports('surface.window.fullChrome');
          const chrome = wantsFull ? ("full" as const) : ("system" as const);
          return lx.surface.openPage("surface", {
            as: "window",
            chrome,
            key: DEMO_PAGE_SURFACE_KEY,
            size,
          });
        }
        return lx.surface.openPage("surface", {
          as: "float",
          position: cfg.position ?? "center",
          key: DEMO_PAGE_SURFACE_KEY,
          size,
        });
      },
      observeDemoPageSurface(surface: PageSurface) {
        surface.onMessage((payload) => {
          // Messages from the surface page no longer auto-close it — that would
          // defeat the show/hide demo.
          const message =
            payload && typeof payload === "object"
              ? (payload as { message?: unknown }).message || JSON.stringify(payload)
              : payload;
          const text = typeof message === "string" ? message : JSON.stringify(message);
          thisPage.update(d => { d.surfaceDemo.message = `Message: ${text}`; });
        });
        // Both opener-side and page-side toggles flow through these events, so the
        // parent UI stays in sync even when the surface hides itself.
        surface.onShow((event) => {
          thisPage.update(d => {
            d.surfaceDemo = {
              ...thisPage.data.surfaceDemo,
              visible: true,
              message: `Shown (source=${event.source})`,
            };
          });
        });
        surface.onHide((event) => {
          thisPage.update(d => {
            d.surfaceDemo = {
              ...thisPage.data.surfaceDemo,
              visible: false,
              message: `Hidden (source=${event.source})`,
            };
          });
        });
        surface.onClose((event) => {
          const currentMessage = thisPage.data.surfaceDemo?.message || "";
          const closeMessage = `Closed: ${event.reason}`;
          thisPage.update(d => {
            d.surfaceDemo = {
              ...thisPage.data.surfaceDemo,
              message: currentMessage.startsWith("Message:")
                ? `${currentMessage} (${closeMessage})`
                : closeMessage,
              active: false,
              visible: false,
            };
          });
        });
      },
      // NavigationBar API functions
      async runChromeUpdate(label: string, update: () => Promise<boolean | void>) {
        try {
          const result = await update();
          thisPage.update(d => { d.chromeError = ""; });
          return result;
        } catch (error) {
          const message = surfaceErrorMessage(error);
          thisPage.update(d => { d.chromeError = `${label}: ${message}`; });
          console.error(`${label} failed:`, error);
          return undefined;
        }
      },
    };
    const actions = {
      bumpLogicCounter() {
        thisPage.update(d => { d.logicCounter = thisPage.data.logicCounter + 1; });
      },
      bumpModuleCounter() {
        moduleCounter += 1;
        thisPage.update(d => { d.moduleCounter = moduleCounter; });
      },
      async setAppearance(options: { preference?: "auto" | "light" | "dark" } = {}) {
        const preference = options.preference || "auto";
        const applied = await helpers.runChromeUpdate("Appearance update", () =>
          lx.host.appearance.setPreference(preference),
        );
        helpers.syncAppearance();
        return applied;
      },
      // Push THIS page again: every entry is its own instance, so the stack
      // drills as deep as the 10-entry cap allows.
      async demoNavigateTo() {
        try {
          await lx.nav.go({ page: "ui", params: { type: "navigation" } }).opened;
        } catch (err) {
          lx.ui.toast({ title: "Page stack is full (10)", icon: "error", durationMs: 1800 });
          console.warn("[UI] navigateTo rejected", err);
        }
      },
      async demoNavigateBack() {
        await lx.nav.back(1);
      },
      async demoSwitchTab() {
        await lx.nav.go({
          tab: "home",
        });
      },
      async demoRedirectTo() {
        await lx.nav.go({
          page: "ui",
          params: { type: "navigation" },
        }, { replace: true });
      },
      // Show toast with custom parameters
      async showToastWithParams(params: Partial<ToastOptions>) {
        const icon = params.icon || thisPage.data.toastIcon || "success";
        const position = params.position || thisPage.data.toastPosition || "center";
        lastToast = await lx.ui.toast({
          title: params.title || "Hello Toast!",
          icon,
          durationMs: params.durationMs || 2000,
          position,
          mask: params.mask || false,
        });
      },
      // Choose toast icon via action sheet
      async chooseToastIcon() {
        const result = await lx.ui.actionSheet({
          items: thisPage.data.toastIconOptions.map((option, index) => ({ id: String(index), label: option.label })),
        });
        if (result.status === 'canceled') {
          return;
        }
        const selected = thisPage.data.toastIconOptions[Number(result.id)];
        thisPage.update(d => {
          d.toastIcon = selected.value;
          d.toastIconLabel = selected.label;
        });
      },
      // Choose toast position via action sheet
      async chooseToastPosition() {
        const result = await lx.ui.actionSheet({
          items: thisPage.data.toastPositionOptions.map((option, index) => ({ id: String(index), label: option.label })),
        });
        if (result.status === 'canceled') {
          return;
        }
        const selected = thisPage.data.toastPositionOptions[Number(result.id)];
        thisPage.update(d => {
          d.toastPosition = selected.value;
          d.toastPositionLabel = selected.label;
        });
      },
      async hideToast() {
        await lastToast?.dismiss();
      },
      // Demo action sheet with mixed language options
      async showDemoActionSheet() {
        thisPage.update(d => { d.actionSheetResult = null; });
        const items = ["View Details", "查看日志", "Send Email", "删除"];
        const result = await lx.ui.actionSheet({
          items: items.map((label, index) => ({ id: String(index), label })),
        });
        thisPage.update(d => { d.actionSheetResult = result; });
        if (result.status === 'canceled') {
          lx.ui.toast({ title: "Dismissed", icon: "none" });
          return;
        }
        lx.ui.toast({ title: `Selected: ${items[Number(result.id)]}`, icon: "success" });
      },
      async openSurfaceDemo(config?: DemoSurfaceConfig) {
        thisPage.update(d => { d.surfaceDemo.message = ""; });

        const cfg = config || {};
        // The content source picks the function; `as` is a small closed set within
        // it. float/window target this app's own page, an aside carries external
        // content, and another lxapp is shell composition.
        const verb = cfg.verb === "float" || cfg.verb === "window" || cfg.verb === "lxapp"
          ? cfg.verb
          : "aside";

        // Ask before offering: a window is a property of the host build, so this
        // answer is stable and does not need the error path to discover it.
        if (verb === "window" && !lx.supports('surface.window')) {
          thisPage.update(d => {
            d.surfaceDemo = {
              ...thisPage.data.surfaceDemo,
              message: "not supported",
              active: false,
              visible: false,
            };
          });
          lx.ui.toast({ title: "not supported", icon: "none" });
          return;
        }

        const size: { width?: number; height?: number } = {};
        if (cfg.width) size.width = cfg.width;
        if (cfg.height) size.height = cfg.height;

        try {
          const surface = await helpers.openDemoSurface(verb, cfg, size);
          // Aside tabs accumulate (multi-tab); only a single float/window drives
          // the hide/show/close controls below.
          const single = verb === "float" || verb === "window";
          thisPage.update(d => {
            d.surfaceDemo = {
              ...thisPage.data.surfaceDemo,
              message: `Opened ${surface.kind} as ${surface.realized}`,
              active: single,
              visible: single,
            };
          });
          if (surface.kind === "page") {
            helpers.observeDemoPageSurface(surface);
          }
        } catch (error) {
          // Every rejection carries a code from the exported union; read it with
          // the helper rather than reaching into the error's shape.
          const message = surfaceErrorMessage(error);
          console.error("lx.surface open failed:", error);
          thisPage.update(d => {
            d.surfaceDemo = {
              ...thisPage.data.surfaceDemo,
              message: `Failed (${surfaceErrorCode(error) ?? "unknown"}): ${message}`,
              active: false,
              visible: false,
            };
          });
          lx.ui.toast({ title: `open failed: ${message}`, icon: "none" });
        }
      },
      async showActiveSurface() {
        // `lx.surface.get(key)` finds the live surface, so no handle is cached.
        const surface = lx.surface.get(DEMO_PAGE_SURFACE_KEY);
        // The demo only ever opens its own page as a surface.
        if (surface?.kind !== "page") {
          return;
        }
        try {
          await surface.show();
          thisPage.update(d => {
            d.surfaceDemo = {
              ...thisPage.data.surfaceDemo,
              message: `Shown ${DEMO_PAGE_SURFACE_KEY}`,
              visible: true,
            };
          });
        } catch (error) {
          console.warn("surface.show failed:", error);
          thisPage.update(d => { d.surfaceDemo.message = `Show failed: ${errorMessage(error, "unknown error")}`; });
        }
      },
      async hideActiveSurface() {
        const surface = lx.surface.get(DEMO_PAGE_SURFACE_KEY);
        if (surface?.kind !== "page") {
          return;
        }
        try {
          await surface.hide();
          thisPage.update(d => {
            d.surfaceDemo = {
              ...thisPage.data.surfaceDemo,
              message: `Hidden ${DEMO_PAGE_SURFACE_KEY}`,
              visible: false,
            };
          });
        } catch (error) {
          console.warn("surface.hide failed:", error);
          thisPage.update(d => { d.surfaceDemo.message = `Hide failed: ${errorMessage(error, "unknown error")}`; });
        }
      },
      async closeActiveSurface() {
        const surface = lx.surface.get(DEMO_PAGE_SURFACE_KEY);
        if (!surface || !("close" in surface)) {
          return;
        }
        try {
          await surface.close();
        } catch (error) {
          console.warn("surface.close failed:", error);
        }
      },
      // Show modal with custom parameters
      async showModalWithParams(params: { title?: string; content?: string; showCancel?: boolean; cancelText?: string; confirmText?: string }) {
        const title = params.title ?? "Alert";
        const message = params.content || "This is a modal dialog";
        const confirmText = params.confirmText || "OK";
        let confirmed = true;
        if (params.showCancel ?? true) {
          confirmed = await lx.ui.confirm({ title, message, confirmText, cancelText: params.cancelText || "Cancel" });
        } else {
          await lx.ui.alert({ title, message, confirmText });
        }
        const result = confirmed ? { status: 'ok' as const } : { status: 'canceled' as const };
        thisPage.update(d => { d.modalResult = result; });
        return result;
      },
      // Clear modal result
      clearModalResult() {
        thisPage.update(d => { d.modalResult = null; });
      },
      updateNavigationBarTitle(options: { title: string }) {
        return helpers.runChromeUpdate("Navigation bar update", () =>
          thisPage.navigationBar.update({ title: options.title }),
        );
      },
      updateNavigationBarColors(options: {
        backgroundColor?: string;
        frontColor?: string;
        dividerColor?: string;
      }) {
        return helpers.runChromeUpdate("Navigation bar update", () =>
          thisPage.navigationBar.update({
            style: {
              backgroundColor: options.backgroundColor,
              foregroundColor: options.frontColor,
              dividerColor: options.dividerColor,
            },
          }),
        );
      },
      updateNavigationBarHomeButton(options: { homeButton: NavigationBarPatch["homeButton"] }) {
        return helpers.runChromeUpdate("Navigation bar update", () =>
          thisPage.navigationBar.update({ homeButton: options.homeButton }),
        );
      },
      resetNavigationBar() {
        return helpers.runChromeUpdate("Navigation bar update", () =>
          thisPage.navigationBar.update({ title: null, style: null, homeButton: "auto" }),
        );
      },
      // TabBar API functions
      enableTabBarRedDot(options: { index: number }) {
        return helpers.runChromeUpdate("Tab bar update", () =>
          lx.app.tabBar.update({ items: [{ index: options.index, redDot: true }] }),
        );
      },
      disableTabBarRedDot(options: { index: number }) {
        return helpers.runChromeUpdate("Tab bar update", () =>
          lx.app.tabBar.update({ items: [{ index: options.index, redDot: false }] }),
        );
      },
      updateTabBarBadge(options: { index: number; text: string }) {
        return helpers.runChromeUpdate("Tab bar update", () =>
          lx.app.tabBar.update({ items: [{ index: options.index, badge: options.text }] }),
        );
      },
      clearTabBarBadge(options: { index: number }) {
        return helpers.runChromeUpdate("Tab bar update", () =>
          lx.app.tabBar.update({ items: [{ index: options.index, badge: null }] }),
        );
      },
      revealTabBar() {
        return helpers.runChromeUpdate("Tab bar update", () =>
          lx.app.tabBar.update({ visibility: "visible" }),
        );
      },
      concealTabBar() {
        return helpers.runChromeUpdate("Tab bar update", () =>
          lx.app.tabBar.update({ visibility: "hidden" }),
        );
      },
      updateTabBarItem(options: TabBarItemPatch) {
        console.log("updateTabBarItem called with:", options);
        return helpers.runChromeUpdate("Tab bar update", () =>
          lx.app.tabBar.update({ items: [options] }),
        );
      },
    };
    return {
      onLoad(options = {}) {
        console.log("UI page onLoad options:", options);

        // Pass querystring parameters to page via setData
        const { type = "navigation" } = (options || {}) as { type?: string };
        if (type !== thisPage.data.currentType) {
          thisPage.update(d => { d.currentType = type; });
        }

        const title = NAV_TITLE_MAP[type] || "User Interface";
        actions.updateNavigationBarTitle({ title });

        helpers.syncAppearance();

        // Instance identity: a fresh tag per entry proves that leaving ends the
        // instance. The previous tag is mirrored through app-scoped storage,
        // which survives the reset.
        const tag = newInstanceTag();
        thisPage.update(d => {
          d.instanceTag = tag;
          d.moduleCounter = moduleCounter;
        });
        helpers.record("onLoad");
        const storage = lx.storage();
        storage
          .get(LAST_INSTANCE_TAG_KEY)
          .then((previous) => {
            thisPage.update(d => { d.previousInstanceTag = (previous as string) || "—"; });
            return storage.set(LAST_INSTANCE_TAG_KEY, tag);
          })
          .catch((err) => console.warn("[UI] instance tag storage failed", err));
      },
      onReady() {
        helpers.record("onReady");
      },
      onShow() {
        console.log("UI page onShow");
        helpers.syncAppearance();
        helpers.record("onShow");
      },
      onHide() {
        console.log("UI page onHide");
        helpers.record("onHide");
      },
      onUnload() {
        helpers.record("onUnload");
      },
      actions,
    };
  },
});
