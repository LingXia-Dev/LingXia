import { showcaseApp } from "../../shared/lib/app";
import {
  resolveDisplayLanguage,
  resolveDisplayLanguagePreference,
  type DisplayLanguagePreference,
} from "../../shared/display-language";
import { formatHomeGreeting, getMessages } from "./messages";
import type { HomeActions, HomePageData, GreetingRequest, AppearanceRequest, DisplayLanguageRequest } from '../../shared/home-types';
import { definePage } from '@lingxia/types/page';


const app = showcaseApp();

const globalData = app.globalData;
const initialData: HomePageData = {
  greeting: globalData.greeting,
  imageUrl: 'https://cn.bing.com/th?id=OHR.BulgariaRocks_EN-US3184562282_UHD.jpg',
  ipAddr: globalData.ipAddr,
  greetCount: 0,
  appVersion: '',
  appearance: { preference: 'auto', resolved: 'light' },
  displayLanguage: { preference: 'auto', resolved: 'en-US' },
};

export default definePage({
  data: initialData,
  setup(page) {
    let ipReadyCallback = null as ((ip: string) => void) | null;
    let stopWatchingAppearance = null as (() => void) | null;
    let stopWatchingLanguage = null as (() => void) | null;
    let stopWatchingLanguagePreference = null as (() => void) | null;
    const helpers = {
      // The product's light/dark scheme. Showcase is its host's Control app, so it
      // owns the setting; the scheme it renders in is read separately, because an
      // lxapp that pinned one in its manifest does not follow the product.
      syncAppearance() {
        try {
          page.update(d => {
            d.appearance = {
              preference: lx.host.appearance.getPreference() ?? "auto",
              resolved: lx.host.appearance.get(),
            };
          });
        } catch (error) {
          console.warn("[Home] Appearance unavailable:", error);
        }
      },
      syncLanguage() {
        try {
          const resolved = lx.host.displayLanguage.get();
          const patch: {
            displayLanguage: {
              preference: DisplayLanguagePreference;
              resolved: string;
            };
            greeting?: string;
          } = {
            displayLanguage: {
              preference: resolveDisplayLanguagePreference(
                lx.host.displayLanguage.getPreference(),
              ),
              resolved,
            },
          };
          if (page.data.greetCount === 0) {
            patch.greeting = getMessages(resolveDisplayLanguage(resolved)).t("defaultGreeting");
          }
          page.update(d => { Object.assign(d, patch); });
        } catch (error) {
          console.warn("[Home] Display language unavailable:", error);
        }
      },
    };
    const actions = {
      async openApi() { await lx.nav.go({ page: 'api' }).opened; },
      async openTodo() { await lx.nav.go({ page: 'todo' }).opened; },
      async openClipboard() { await lx.nav.go({ page: 'clipboard' }).opened; },
      async openShare() { await lx.nav.go({ page: 'share' }).opened; },
      async setAppearance(options: Partial<AppearanceRequest> = {}) {
        const preference = options.preference || "auto";
        try {
          await lx.host.appearance.setPreference(preference);
        } catch (error) {
          console.warn("[Home] Failed to set appearance:", error);
          const { t } = getMessages(resolveDisplayLanguage(lx.host.displayLanguage.get()));
          lx.ui.toast({ title: t("appearanceUnavailable"), icon: "none" });
        }
        helpers.syncAppearance();
      },
      // Showcase is the Control app, so it owns the product-wide language
      // preference. Ordinary lxapps must not add their own selector.
      async setDisplayLanguage(
        options: Partial<DisplayLanguageRequest> = {},
      ) {
        const preference = options.preference || "auto";
        try {
          await lx.host.displayLanguage.setPreference(preference);
        } catch (error) {
          console.warn("[Home] Failed to set display language:", error);
          const { t } = getMessages(resolveDisplayLanguage(lx.host.displayLanguage.get()));
          lx.ui.toast({ title: t("languageUnavailable"), icon: "none" });
        }
        helpers.syncLanguage();
      },
      async greet(option: Partial<GreetingRequest> = {}) {
        const name = typeof option.name === "string" && option.name ? option.name : "LingXia";
        const count = page.data.greetCount + 1;
        page.update(d => {
          d.greeting = formatHomeGreeting(
            resolveDisplayLanguage(lx.host.displayLanguage.get()),
            name,
            count,
          );
          d.greetCount = count;
        });
        await page.flush();
        console.log("[Home] greeting acknowledged by the View");
      },
    };
    return {
      onReady() {
        console.log("[Home] Page ready");
        const callback = (ip: string) => {
          if (ipReadyCallback !== callback) return;
          console.log("IP received in Page:", ip);
          page.update(d => { d.ipAddr = ip; });
        };
        ipReadyCallback = callback;
        app.ipReadyCallback = callback;

        if (app.globalData.ipAddr) {
          page.update(d => { d.ipAddr = app.globalData.ipAddr; });
        }
      },
      onUnload() {
        console.log("[Home] Page unloaded");
        stopWatchingAppearance?.();
        stopWatchingAppearance = null;
        stopWatchingLanguage?.();
        stopWatchingLanguage = null;
        stopWatchingLanguagePreference?.();
        stopWatchingLanguagePreference = null;
        if (app.ipReadyCallback === ipReadyCallback) {
          app.ipReadyCallback = undefined;
        }
        ipReadyCallback = null;
      },
      async onLoad() {
        // Keep the exported View surface checked against registered Logic actions.
        void (actions satisfies HomeActions);
        console.log("[Home] Page loaded");
        helpers.syncAppearance();
        helpers.syncLanguage();
        stopWatchingAppearance = lx.host.appearance.watch(() => helpers.syncAppearance());
        stopWatchingLanguage = lx.host.displayLanguage.watch(() => helpers.syncLanguage());
        stopWatchingLanguagePreference =
          lx.host.displayLanguage.watchPreference(() => helpers.syncLanguage()) ?? null;
        try {
          const info = lx.app.info;
          const suffix =
            info.channel && info.channel !== "release"
              ? ` (${info.channel})`
              : "";
          page.update(d => { d.appVersion = `v${info.version}${suffix}`; });
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
      onHide() {
        console.log("[Home] Page hidden");
      },
      onShow() {
        console.log("[Home] Page shown");
        console.log("[Home] App data:", app.globalData);
        helpers.syncAppearance();
        helpers.syncLanguage();
      },
      actions,
    };
  },
});
