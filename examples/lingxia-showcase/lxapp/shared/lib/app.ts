import type { AppInstance, AppLaunchOptions } from "@lingxia/types";

/** What `App({...})` in `lxapp.ts` adds on top of the runtime instance. */
export interface ShowcaseAppInstance extends AppInstance {
  globalData: {
    lifecycle: {
      launches: number;
      shows: number;
      hides: number;
      last: string;
      /** Home page `onHide` / `onShow` counts, for lifecycle probes in specs. */
      pageHides: number;
      pageShows: number;
      /** The last App `onShow` options, for reopen probes in specs. */
      lastShow?: AppLaunchOptions;
    };
    greeting: string;
    ipAddr: string;
    /** Written by the Wi-Fi page so the flag survives navigation away and back. */
    wifiModuleEnabled?: boolean;
  };
  ipReadyCallback?: (ip: string) => void;
}

/**
 * `getApp()` is typed nullable because a page can ask before the app exists.
 * Inside a page callback it always does, so the pages read it through here
 * rather than repeating a null check that can never fail.
 */
export function showcaseApp(): ShowcaseAppInstance {
  const app = getApp<ShowcaseAppInstance>();
  if (!app) throw new Error("App instance is not available");
  return app;
}
