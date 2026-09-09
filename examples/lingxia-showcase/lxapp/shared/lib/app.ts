import type { AppInstance } from "@lingxia/types";

/** What `App({...})` in `lxapp.ts` adds on top of the runtime instance. */
export interface ShowcaseAppInstance extends AppInstance {
  globalData: {
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
