import { showcaseApp } from "../../shared/lib/app";
import {
  resolveDisplayLanguage,
  resolveDisplayLanguagePreference,
  type DisplayLanguagePreference,
} from "../../shared/display-language";
import { formatHomeGreeting, getMessages } from "./messages";
import { applyShowcaseTabBar } from "../../logic/app-messages";


const app = showcaseApp();

const globalData = app.globalData;

Page({
  ipReadyCallback: null as ((ip: string) => void) | null,
  stopWatchingAppearance: null as (() => void) | null,
  stopWatchingLanguage: null as (() => void) | null,
  stopWatchingLanguagePreference: null as (() => void) | null,

  data: {
    greeting: globalData.greeting,
    imageUrl:
      "https://cn.bing.com/th?id=OHR.BulgariaRocks_EN-US3184562282_UHD.jpg",

    ipAddr: globalData.ipAddr,
    greetCount: 0,
    appVersion: "",
    appearance: { preference: "auto", resolved: "light" },
    displayLanguage: {
      preference: "auto" as DisplayLanguagePreference,
      resolved: "en-US",
    },
  },

  // The product's light/dark scheme. Showcase is its host's Control app, so it
  // owns the setting; the scheme it renders in is read separately, because an
  // lxapp that pinned one in its manifest does not follow the product.
  _syncAppearance: function () {
    try {
      this.setData({
        appearance: {
          preference: lx.app.control?.appearance.getPreference() ?? "auto",
          resolved: lx.app.appearance.get(),
        },
      });
    } catch (error) {
      console.warn("[Home] Appearance unavailable:", error);
    }
  },

  _syncLanguage: function () {
    try {
      const resolved = lx.app.displayLanguage.get();
      const patch: {
        displayLanguage: {
          preference: DisplayLanguagePreference;
          resolved: string;
        };
        greeting?: string;
      } = {
        displayLanguage: {
          preference: resolveDisplayLanguagePreference(
            lx.app.control?.displayLanguage.getPreference(),
          ),
          resolved,
        },
      };
      if (this.data.greetCount === 0) {
        patch.greeting = getMessages(resolveDisplayLanguage(resolved)).t("defaultGreeting");
      }
      this.setData(patch);
    } catch (error) {
      console.warn("[Home] Display language unavailable:", error);
    }
  },

  setAppearance: async function (options: { preference?: "auto" | "light" | "dark" } = {}) {
    const preference = options.preference || "auto";
    try {
      await lx.app.control?.appearance.setPreference(preference);
    } catch (error) {
      console.warn("[Home] Failed to set appearance:", error);
      const { t } = getMessages(resolveDisplayLanguage(lx.app.displayLanguage.get()));
      lx.showToast({ title: t("appearanceUnavailable"), icon: "none" });
    }
    this._syncAppearance();
  },

  // Showcase is the Control app, so it owns the product-wide language
  // preference. Ordinary lxapps must not add their own selector.
  setDisplayLanguage: async function (
    options: { preference?: DisplayLanguagePreference } = {},
  ) {
    const preference = options.preference || "auto";
    try {
      await lx.app.control?.displayLanguage.setPreference(preference);
    } catch (error) {
      console.warn("[Home] Failed to set display language:", error);
      const { t } = getMessages(resolveDisplayLanguage(lx.app.displayLanguage.get()));
      lx.showToast({ title: t("languageUnavailable"), icon: "none" });
    }
    this._syncLanguage();
    void applyShowcaseTabBar().catch((error) =>
      console.warn("[Home] tab bar language update failed", error),
    );
  },

  onReady: function () {
    console.log("[Home] Page ready");
    const callback = (ip: string) => {
      if (this.ipReadyCallback !== callback) return;
      console.log("IP received in Page:", ip);
      this.setData({
        ipAddr: ip,
      });
    };
    this.ipReadyCallback = callback;
    app.ipReadyCallback = callback;

    if (app.globalData.ipAddr) {
      this.setData({
        ipAddr: app.globalData.ipAddr,
      });
    }
  },

  onUnload: function () {
    console.log("[Home] Page unloaded");
    this.stopWatchingAppearance?.();
    this.stopWatchingAppearance = null;
    this.stopWatchingLanguage?.();
    this.stopWatchingLanguage = null;
    this.stopWatchingLanguagePreference?.();
    this.stopWatchingLanguagePreference = null;
    if (app.ipReadyCallback === this.ipReadyCallback) {
      app.ipReadyCallback = undefined;
    }
    this.ipReadyCallback = null;
  },

  onLoad: async function () {
    console.log("[Home] Page loaded");
    this._syncAppearance();
    this._syncLanguage();
    this.stopWatchingAppearance = lx.app.appearance.watch(() => this._syncAppearance());
    this.stopWatchingLanguage = lx.app.displayLanguage.watch(() => this._syncLanguage());
    this.stopWatchingLanguagePreference =
      lx.app.control?.displayLanguage.watchPreference(() => this._syncLanguage()) ?? null;
    try {
      const info = lx.getLxAppInfo();
      const suffix =
        info.channel && info.channel !== "release"
          ? ` (${info.channel})`
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

  onHide: function () {
    console.log("[Home] Page hidden");
  },

  onShow: function () {
    console.log("[Home] Page shown");
    console.log("[Home] App data:", app.globalData);
    this._syncAppearance();
    this._syncLanguage();
  },

  greet: function (option: { name?: string } = {}) {

    const name = typeof option.name === "string" && option.name ? option.name : "LingXia";
    const count = this.data.greetCount + 1;
    this.setData(
      {
        greeting: formatHomeGreeting(
          resolveDisplayLanguage(lx.app.displayLanguage.get()),
          name,
          count,
        ),
        greetCount: count,
      },
      () => {
        console.log("setData callback");
      },
    );
  },
});
