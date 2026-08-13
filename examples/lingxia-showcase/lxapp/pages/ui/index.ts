import { surfaceErrorCode } from "lingxia-types/error";

const app = getApp();

const DEMO_SURFACE_KEY = "surface-demo";

const NAV_TITLE_MAP = {
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

Page({
  data: {
    currentType: "navigation",
    pageStack: [],
    instanceTag: "",
    previousInstanceTag: "",
    logicCounter: 0,
    moduleCounter: 0,
    events: [] as string[],
    modalResult: null,
    toastIcon: "success",
    toastIconLabel: "Success",
    toastIconOptions: [
      { label: "Success", value: "success" },
      { label: "Error", value: "error" },
      { label: "Loading", value: "loading" },
      { label: "None", value: "none" },
    ],
    toastPosition: "center",
    toastPositionLabel: "Center",
    toastPositionOptions: [
      { label: "Top", value: "top" },
      { label: "Center", value: "center" },
      { label: "Bottom", value: "bottom" },
    ],
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

  onLoad: function (options = {}) {
    console.log("UI page onLoad options:", options);

    // Pass querystring parameters to page via setData
    const { type = "navigation" } = (options || {}) as { type?: string };
    if (type !== this.data.currentType) {
      this.setData({ currentType: type });
    }

    const title = NAV_TITLE_MAP[type] || "User Interface";
    this.updateNavigationBarTitle({ title });

    // Update page stack immediately
    this._updatePageStack();
    this._syncAppearance();

    // Instance identity: a fresh tag per entry proves that leaving ends the
    // instance. The previous tag is mirrored through app-scoped storage,
    // which survives the reset.
    const tag = newInstanceTag();
    this.setData({ instanceTag: tag, moduleCounter });
    this._record("onLoad");
    const storage = lx.getStorage();
    storage
      .get(LAST_INSTANCE_TAG_KEY)
      .then((previous) => {
        this.setData({ previousInstanceTag: (previous as string) || "—" });
        return storage.set(LAST_INSTANCE_TAG_KEY, tag);
      })
      .catch((err) => console.warn("[UI] instance tag storage failed", err));
  },

  onReady: function () {
    this._record("onReady");
  },

  onShow: function () {
    console.log("UI page onShow");
    // Update page stack every time page shows
    this._updatePageStack();
    this._syncAppearance();
    this._record("onShow");
  },

  onHide: function () {
    console.log("UI page onHide");
    this._record("onHide");
  },

  onUnload: function () {
    this._record("onUnload");
  },

  bumpLogicCounter: function () {
    this.setData({ logicCounter: this.data.logicCounter + 1 });
  },

  bumpModuleCounter: function () {
    moduleCounter += 1;
    this.setData({ moduleCounter });
  },

  // The log lives in `data`, so it only ever shows the current instance's
  // events — hide/show repeat within one instance, unload ends it.
  _record: function (event: string) {
    const stamp = new Date().toLocaleTimeString();
    this.setData({
      events: [...this.data.events, `${stamp}  ${event}`].slice(-MAX_EVENTS),
    });
  },

  // Appearance is part of Page Chrome: the lxapp picks its own light/dark branch
  // and the runtime projects the resolved one into every page as `color-scheme`
  // plus `data-theme` on <html>. The preference persists per lxapp, so it is
  // re-read from the runtime instead of being mirrored in page state.
  _syncAppearance: function () {
    try {
      this.setData({ appearance: lx.appearance.get() });
    } catch (error) {
      this.setData({ chromeError: `Appearance unavailable: ${surfaceErrorMessage(error)}` });
    }
  },

  setAppearance: async function (options: { preference?: "auto" | "light" | "dark" } = {}) {
    const preference = options.preference || "auto";
    const applied = await this._runChromeUpdate("Appearance update", () =>
      lx.appearance.set(preference),
    );
    this._syncAppearance();
    return applied;
  },

  // Update current page stack
  _updatePageStack: function () {
    try {
      const pages = getCurrentPages();
      const top = pages.length - 1;
      const pageStack = pages.map((page, index) => ({
        index,
        name: ((page.route || "unknown").split("pages/")[1] || page.route || "unknown")
          .replace(/\/index\.\w+$/, ""),
        current: index === top,
      }));

      this.setData({
        pageStack: pageStack,
      });
    } catch (error) {
      console.error("Failed to get current pages:", error);
    }
  },

  // Push THIS page again: every entry is its own instance, so the stack
  // drills as deep as the 10-entry cap allows.
  demoNavigateTo: async function () {
    try {
      await lx.navigateTo({ page: "ui", query: { type: "navigation" } });
    } catch (err) {
      lx.showToast({ title: "Page stack is full (10)", icon: "error", duration: 1800 });
      console.warn("[UI] navigateTo rejected", err);
    }
  },

  demoNavigateBack: async function () {
    await lx.navigateBack({
      delta: 1,
    });
  },

  demoSwitchTab: async function () {
    await lx.switchTab({
      page: "home",
    });
  },

  demoRedirectTo: async function () {
    await lx.redirectTo({
      page: "ui",
      query: { type: "navigation" },
    });
  },

  // Show toast with custom parameters
  showToastWithParams: function (params) {
    const icon = params.icon || this.data.toastIcon || "success";
    const position = params.position || this.data.toastPosition || "center";
    lx.showToast({
      title: params.title || "Hello Toast!",
      icon,
      duration: params.duration || 2000,
      position,
      mask: params.mask || false,
    });
  },

  // Choose toast icon via action sheet
  chooseToastIcon: async function () {
    const result = await lx.showActionSheet({
      itemList: this.data.toastIconOptions.map((option) => option.label),
      itemColor: "#007AFF",
    });
    if (result.canceled) {
      return;
    }
    const selected = this.data.toastIconOptions[result.index];
    this.setData({ toastIcon: selected.value, toastIconLabel: selected.label });
  },

  // Choose toast position via action sheet
  chooseToastPosition: async function () {
    const result = await lx.showActionSheet({
      itemList: this.data.toastPositionOptions.map((option) => option.label),
      itemColor: "#007AFF",
    });
    if (result.canceled) {
      return;
    }
    const selected = this.data.toastPositionOptions[result.index];
    this.setData({ toastPosition: selected.value, toastPositionLabel: selected.label });
  },

  hideToast: function () {
    lx.hideToast();
  },

  // Demo action sheet with mixed language options
  showDemoActionSheet: async function () {
    const items = ["View Details", "查看日志", "Send Email", "删除"];
    const result = await lx.showActionSheet({
      itemList: items,
      itemColor: "#007AFF",
    });
    if (result.canceled) {
      lx.showToast({ title: "Dismissed", icon: "none" });
      return;
    }
    lx.showToast({ title: `Selected: ${items[result.index]}`, icon: "success" });
  },

  openSurfaceDemo: async function (config) {
    this.setData({ "surfaceDemo.message": "" });

    const cfg = config || {};
    // The content source picks the function; `as` is a small closed set within
    // it. float/window target this app's own page, an aside carries external
    // content, and another lxapp is shell composition.
    const verb = cfg.verb === "float" || cfg.verb === "window" || cfg.verb === "lxapp"
      ? cfg.verb
      : "aside";

    // Ask before offering: a window is a property of the host build, so this
    // answer is stable and does not need the error path to discover it.
    if (verb === "window" && !lx.supports({ surface: "window" })) {
      this.setData({
        "surfaceDemo.message": "not supported",
        "surfaceDemo.active": false,
        "surfaceDemo.visible": false,
      });
      lx.showToast({ title: "not supported", icon: "none" });
      return;
    }

    const size = {};
    if (cfg.width) size.width = cfg.width;
    if (cfg.height) size.height = cfg.height;

    try {
      const surface = await this._openDemoSurface(verb, cfg, size);
      // Aside tabs accumulate (multi-tab); only a single float/window drives
      // the hide/show/close controls below.
      const single = verb === "float" || verb === "window";
      this.setData({
        "surfaceDemo.message": `Opened ${surface.realized}: ${surface.id}`,
        "surfaceDemo.active": single,
        "surfaceDemo.visible": single,
      });
      if (surface.kind === "page") {
        this._observeDemoPageSurface(surface);
      }
    } catch (error) {
      // Every rejection carries a code from the exported union; read it with
      // the helper rather than reaching into the error's shape.
      const message = surfaceErrorMessage(error);
      console.error("lx.surface open failed:", error);
      this.setData({
        "surfaceDemo.message": `Failed (${surfaceErrorCode(error) ?? "unknown"}): ${message}`,
        "surfaceDemo.active": false,
        "surfaceDemo.visible": false,
      });
      lx.showToast({ title: `open failed: ${message}`, icon: "none" });
    }
  },

  _openDemoSurface: function (verb, cfg, size) {
    if (verb === "lxapp") {
      // Compose another lxapp into the aside slot — home-lxapp privilege, so
      // it lives on lx.shell rather than lx.surface.
      return lx.shell.openApp("lingxia-chat", {
        as: "aside",
        edge: cfg.edge ?? "right",
        key: DEMO_SURFACE_KEY,
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
      const idx = (this._asideTabIndex || 0) % demoUrls.length;
      this._asideTabIndex = (this._asideTabIndex || 0) + 1;
      return lx.surface.openUrl(cfg.url ?? demoUrls[idx], {
        as: "aside",
        edge: cfg.edge ?? "right",
        size,
      });
    }
    if (verb === "window") {
      // Edge-to-edge when the host build can keep the system controls; the
      // page lays out under the runtime's drag strip via the page-chrome
      // top inset, so it never has to opt in to stay movable.
      const chrome = lx.supports({ surface: "window", chrome: "full" })
        ? ("full" as const)
        : ("system" as const);
      return lx.surface.openPage("surface", {
        as: "window",
        chrome,
        key: DEMO_SURFACE_KEY,
        size,
      });
    }
    return lx.surface.openPage("surface", {
      as: "float",
      position: cfg.position ?? "center",
      key: DEMO_SURFACE_KEY,
      size,
    });
  },

  _observeDemoPageSurface: function (surface) {
    surface.onMessage((payload) => {
      // Messages from the surface page no longer auto-close it — that would
      // defeat the show/hide demo.
      const message =
        payload && typeof payload === "object"
          ? payload.message || JSON.stringify(payload)
          : payload;
      const text = typeof message === "string" ? message : JSON.stringify(message);
      this.setData({ "surfaceDemo.message": `Message: ${text}` });
    });
    // Both opener-side and page-side toggles flow through these events, so the
    // parent UI stays in sync even when the surface hides itself.
    surface.onShow((event) => {
      this.setData({
        "surfaceDemo.visible": true,
        "surfaceDemo.message": `Shown ${event.id} (source=${event.source})`,
      });
    });
    surface.onHide((event) => {
      this.setData({
        "surfaceDemo.visible": false,
        "surfaceDemo.message": `Hidden ${event.id} (source=${event.source})`,
      });
    });
    surface.onClose((event) => {
      const currentMessage = this.data.surfaceDemo?.message || "";
      const closeMessage = `Closed ${event.id}: ${event.reason}`;
      this.setData({
        "surfaceDemo.message": currentMessage.startsWith("Message:")
          ? `${currentMessage} (${closeMessage})`
          : closeMessage,
        "surfaceDemo.active": false,
        "surfaceDemo.visible": false,
      });
    });
  },

  showActiveSurface: async function () {
    // `lx.surface.get(key)` replaces caching the handle by hand.
    const surface = lx.surface.get(DEMO_SURFACE_KEY);
    if (!surface || surface.kind === "tab") {
      return;
    }
    try {
      await surface.show();
      this.setData({
        "surfaceDemo.message": `Shown ${surface.id}`,
        "surfaceDemo.visible": true,
      });
    } catch (error) {
      console.warn("surface.show failed:", error);
      this.setData({ "surfaceDemo.message": `Show failed: ${error.message}` });
    }
  },

  hideActiveSurface: async function () {
    const surface = lx.surface.get(DEMO_SURFACE_KEY);
    if (!surface || surface.kind === "tab") {
      return;
    }
    try {
      await surface.hide();
      this.setData({
        "surfaceDemo.message": `Hidden ${surface.id}`,
        "surfaceDemo.visible": false,
      });
    } catch (error) {
      console.warn("surface.hide failed:", error);
      this.setData({ "surfaceDemo.message": `Hide failed: ${error.message}` });
    }
  },

  closeActiveSurface: async function () {
    const surface = lx.surface.get(DEMO_SURFACE_KEY);
    if (!surface) {
      return;
    }
    try {
      await surface.close();
    } catch (error) {
      console.warn("surface.close failed:", error);
    }
  },

  // Show modal with custom parameters
  showModalWithParams: async function (params) {
    const result = await lx.showModal({
      title: params.title ?? "Alert",
      content: params.content || "This is a modal dialog",
      showCancel: params.showCancel ?? true,
      cancelText: params.cancelText || "Cancel",
      confirmText: params.confirmText || "OK",
    });
    this.setData({ modalResult: result });
    return result;
  },

  // Clear modal result
  clearModalResult: function () {
    this.setData({
      modalResult: null,
    });
  },

  // NavigationBar API functions
  _runChromeUpdate: async function (label, update) {
    try {
      const result = await update();
      this.setData({ chromeError: "" });
      return result;
    } catch (error) {
      const message = surfaceErrorMessage(error);
      this.setData({ chromeError: `${label}: ${message}` });
      console.error(`${label} failed:`, error);
      return undefined;
    }
  },

  updateNavigationBarTitle: function (options) {
    return this._runChromeUpdate("Navigation bar update", () =>
      lx.navigationBar.update({ title: options.title }),
    );
  },

  updateNavigationBarColors: function (options) {
    return this._runChromeUpdate("Navigation bar update", () =>
      lx.navigationBar.update({
        style: {
          backgroundColor: options.backgroundColor,
          foregroundColor: options.frontColor,
        },
      }),
    );
  },

  // TabBar API functions
  enableTabBarRedDot: function (options) {
    return this._runChromeUpdate("Tab bar update", () =>
      lx.tabBar.update({ items: [{ index: options.index, redDot: true }] }),
    );
  },

  disableTabBarRedDot: function (options) {
    return this._runChromeUpdate("Tab bar update", () =>
      lx.tabBar.update({ items: [{ index: options.index, redDot: false }] }),
    );
  },

  updateTabBarBadge: function (options) {
    return this._runChromeUpdate("Tab bar update", () =>
      lx.tabBar.update({ items: [{ index: options.index, badge: options.text }] }),
    );
  },

  clearTabBarBadge: function (options) {
    return this._runChromeUpdate("Tab bar update", () =>
      lx.tabBar.update({ items: [{ index: options.index, badge: null }] }),
    );
  },

  revealTabBar: function () {
    return this._runChromeUpdate("Tab bar update", () =>
      lx.tabBar.update({ visibility: "auto" }),
    );
  },

  concealTabBar: function () {
    return this._runChromeUpdate("Tab bar update", () =>
      lx.tabBar.update({ visibility: "hidden" }),
    );
  },

  updateTabBarForegrounds: function (options) {
    console.log("updateTabBarForegrounds called with:", options);
    return this._runChromeUpdate("Tab bar update", () =>
      lx.tabBar.update({
        style: {
          foregroundColor: options.color,
          selectedForegroundColor: options.selectedColor,
        },
      }),
    );
  },

  updateTabBarItem: function (options) {
    console.log("updateTabBarItem called with:", options);
    return this._runChromeUpdate("Tab bar update", () =>
      lx.tabBar.update({ items: [options] }),
    );
  },
});
