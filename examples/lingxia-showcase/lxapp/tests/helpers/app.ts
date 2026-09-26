import type { LxAppDriver } from '@lingxia/types/automation';

export const SHOWCASE_APP_ID = 'lingxia-showcase';

/**
 * What the raw-driver helpers use: page, nav, info and string eval. Both the
 * raw `lx.automation().lxapp()` driver and the fixture's `t.app` (through its
 * `page` and `eval({ script })`) provide it.
 */
export type AppDriver = Pick<LxAppDriver, 'page' | 'nav' | 'eval' | 'info' | 'pages' | 'surfaceLayout'>;

export function showcaseApp(): LxAppDriver {
  return lx.automation().lxapp(SHOWCASE_APP_ID);
}
