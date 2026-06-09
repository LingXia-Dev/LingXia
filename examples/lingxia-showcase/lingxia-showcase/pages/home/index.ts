const app = getApp();
const globalData = app.globalData;

Page({
  data: {
    greeting: globalData.greeting,
    imageUrl:
      "https://cn.bing.com/th?id=OHR.BulgariaRocks_EN-US3184562282_UHD.jpg",

    ipAddr: globalData.ipAddr,
    greetCount: 0,
    appVersion: "",
  },

  onReady: function() {
    console.log("[Home] Page ready");
    // Add callback directly to App
    app.ipReadyCallback = (ip) => {
      console.log("IP received in Page:", ip);
      this.setData({
        ipAddr: ip,
      });
    };

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
  },

  onLoad: async function() {
    console.log("[Home] Page loaded");
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
      const files = lx.getFileManager();
      const testFile = "debug/testFile.txt";
      await files.mkdir({ path: "debug", recursive: true });
      await files.writeFile({
        filePath: testFile,
        data: "Hello, World!",
        overwrite: true,
      });
      const { data } = await files.readFile({ filePath: testFile, encoding: "utf8" });
      console.log("[Home] FileManager test content:", data);
    } catch (error) {
      console.warn("[Home] FileManager test failed:", error);
    }
  },

  onHide: function() {
    console.log("[Home] Page hidden");
  },

  onShow: function() {
    console.log("[Home] Page shown");
    console.log("[Home] App data:", app.globalData);
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
