const app = getApp();
const globalData = app.globalData;

Page({
  ipReadyCallback: null as ((ip: string) => void) | null,

  data: {
    greeting: globalData.greeting,
    imageUrl:
      "https://cn.bing.com/th?id=OHR.BulgariaRocks_EN-US3184562282_UHD.jpg",

    ipAddr: globalData.ipAddr,
    greetCount: 0,
    appVersion: "",
    appearance: { preference: "auto", resolved: "light" },
  },

  // The lxapp's own light/dark branch, independent of the host shell. The
  // preference persists per lxapp, so it is re-read on every show rather than
  // tracked locally. Guarded because this also runs from onLoad on the app's
  // first screen: a host that predates lx.appearance must not take it down.
  _syncAppearance: function () {
    try {
      this.setData({ appearance: lx.appearance.get() });
    } catch (error) {
      console.warn("[Home] Appearance unavailable:", error);
    }
  },

  setAppearance: async function (options: { preference?: "auto" | "light" | "dark" } = {}) {
    const preference = options.preference || "auto";
    try {
      await lx.appearance.set(preference);
    } catch (error) {
      console.warn("[Home] Failed to set appearance:", error);
      lx.showToast({ title: "Appearance unavailable", icon: "none" });
    }
    this._syncAppearance();
  },

  onReady: function() {
    console.log("[Home] Page ready");
    // Add callback directly to App
    const callback = (ip: string) => {
      if (this.ipReadyCallback !== callback) return;
      console.log("IP received in Page:", ip);
      this.setData({
        ipAddr: ip,
      });
    };
    this.ipReadyCallback = callback;
    app.ipReadyCallback = callback;

    // Check if IP is already available
    if (app.globalData.ipAddr) {
      (() => {
        this.setData({
          ipAddr: app.globalData.ipAddr,
        });
      })();
    }
  },

  onUnload: function() {
    console.log("[Home] Page unloaded");
    if (app.ipReadyCallback === this.ipReadyCallback) {
      app.ipReadyCallback = undefined;
    }
    this.ipReadyCallback = null;
  },

  onLoad: async function() {
    console.log("[Home] Page loaded");
    this._syncAppearance();
    try {
      const info = lx.getLxAppInfo();
      const suffix =
        info.releaseType && info.releaseType !== "release"
          ? ` (${info.releaseType})`
          : "";
      this.setData({
        appVersion: `v${info.version}${suffix}`,
      });
    } catch (error) {
      console.error("[Home] Failed to get app version:", error);
    }

    try {
      const testFile = "debug/testFile.txt";
      await lx.fs.mkdir("debug", { recursive: true });
      await lx.fs.write(testFile, "Hello, World!", {
        overwrite: true,
      });
      const data = await lx.fs.file(testFile).text();
      console.log("[Home] managed file test content:", data);
    } catch (error) {
      console.warn("[Home] managed file test failed:", error);
    }
  },

  onHide: function() {
    console.log("[Home] Page hidden");
  },

  onShow: function() {
    console.log("[Home] Page shown");
    console.log("[Home] App data:", app.globalData);
    this._syncAppearance();
  },

  greet: function(option = {}) {
    const name = typeof option.name === "string" && option.name ? option.name : "LingXia";
    const count = this.data.greetCount + 1;
    this.setData(
      {
        greeting: `👋 Hello ${name}! (#${count})

🌍 Greetings from appservice powered by Rust and JS engine
🕒 ${new Date().toLocaleTimeString("en-US", { hour: "2-digit", minute: "2-digit", second: "2-digit" })}`,
        greetCount: count,
      },
      () => {
        console.log("setData callback");
      },
    );
  },
});
