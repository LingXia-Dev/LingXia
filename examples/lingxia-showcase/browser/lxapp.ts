import { defineApp, lx, type AppLifecycleEventArgs, type ConfiguredPageName } from '@lingxia/logic';

export default defineApp({
  // A reopen naming a page (the sidebar's Downloads, the host's Settings) lands on it;
  // an open instance is brought back rather than stacked again.
  onShow(args?: AppLifecycleEventArgs) {
    if (args?.reason !== 'switch_back' || !args.path) return;
    const page = /^pages\/([^/]+)\//.exec(args.path)?.[1] as ConfiguredPageName | undefined;
    if (!page || lx.nav.current.get()?.page === page) return;
    void lx.nav.go({ page }, { reuse: true })
      .catch((error) => console.warn(`reopen at ${page} failed`, error));
  },
});
