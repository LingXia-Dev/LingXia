import { appState, watchIpAddr } from "../../shared/lib/app";
import { definePage, lx } from '@lingxia/logic';

type AppearancePreference = 'auto' | 'light' | 'dark';
interface HomePageData {
  /** Who was greeted last, and when (ms); the View words it. */
  greetName: string;
  greetedAt: number;
  imageUrl: string;
  ipAddr: string;
  greetCount: number;
  appVersion: string;
  appearance: { preference: AppearancePreference; resolved: 'light' | 'dark' };
  /** The product's language preference: `'auto'` or a tag. The language shown is `env.language`. */
  languagePreference: string;
}

const initialData: HomePageData = {
  greetName: '',
  greetedAt: 0,
  imageUrl: 'https://cn.bing.com/th?id=OHR.BulgariaRocks_EN-US3184562282_UHD.jpg',
  ipAddr: appState.ipAddr,
  greetCount: 0,
  appVersion: '',
  appearance: { preference: 'auto', resolved: 'light' },
  languagePreference: 'auto',
};

export default definePage({
  data: initialData,
  setup(page) {
    let stopWatchingIp = null as (() => void) | null;
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
              preference: lx.host.appearancePreference.get() ?? "auto",
              resolved: lx.app.appearance.get(),
            };
          });
        } catch (error) {
          console.warn("[Home] Appearance unavailable:", error);
        }
      },
      syncLanguage() {
        try {
          const preference = lx.host.languagePreference.get() ?? "auto";
          page.update(d => { d.languagePreference = preference; });
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
      async setAppearance(options: { preference?: AppearancePreference } = {}) {
        const preference = options.preference || "auto";
        try {
          await lx.host.appearancePreference.set(preference);
        } catch (error) {
          console.warn("[Home] Failed to set appearance:", error);
          lx.ui.toast(lx.app.t("home.appearanceUnavailable"));
        }
        helpers.syncAppearance();
      },
      // Showcase is the Control app, so it owns the product-wide language
      // preference. Ordinary lxapps must not add their own selector.
      async setDisplayLanguage(
        options: { preference?: string } = {},
      ) {
        const preference = options.preference || "auto";
        try {
          await lx.host.languagePreference.set(preference);
        } catch (error) {
          console.warn("[Home] Failed to set display language:", error);
          lx.ui.toast(lx.app.t("home.languageUnavailable"));
        }
        helpers.syncLanguage();
      },
      async greet(option: { name?: string } = {}) {
        const name = typeof option.name === "string" && option.name ? option.name : "LingXia";
        const count = page.data.greetCount + 1;
        page.update(d => {
          d.greetName = name;
          d.greetedAt = Date.now();
          d.greetCount = count;
        });
        await page.flush();
        console.log("[Home] greeting acknowledged by the View");
      },
    };
    return {
      onReady() {
        console.log("[Home] Page ready");
        stopWatchingIp?.();
        stopWatchingIp = watchIpAddr((ip) => {
          page.update(d => { d.ipAddr = ip; });
        });
        page.update(d => { d.ipAddr = appState.ipAddr; });
      },
      onUnload() {
        console.log("[Home] Page unloaded");
        stopWatchingAppearance?.();
        stopWatchingAppearance = null;
        stopWatchingLanguage?.();
        stopWatchingLanguage = null;
        stopWatchingLanguagePreference?.();
        stopWatchingLanguagePreference = null;
        stopWatchingIp?.();
        stopWatchingIp = null;
      },
      async onLoad() {
        console.log("[Home] Page loaded");
        helpers.syncAppearance();
        helpers.syncLanguage();
        stopWatchingAppearance = lx.app.appearance.watch(() => helpers.syncAppearance());
        stopWatchingLanguage = lx.app.language.watch(() => helpers.syncLanguage());
        stopWatchingLanguagePreference =
          lx.host.languagePreference.watch(() => helpers.syncLanguage());
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
          await lx.fs.write(testFile, { data: "Hello, World!", overwrite: true });
          const data = await lx.fs.file(testFile).text();
          console.log("[Home] managed file test content:", data);
        } catch (error) {
          console.warn("[Home] managed file test failed:", error);
        }
      },
      onHide() {
        appState.lifecycle.pageHides++;
        console.log("[Home] Page hidden");
      },
      onShow() {
        appState.lifecycle.pageShows++;
        console.log("[Home] Page shown");
        console.log("[Home] App data:", appState);
        helpers.syncAppearance();
        helpers.syncLanguage();
      },
      actions,
    };
  },
});
