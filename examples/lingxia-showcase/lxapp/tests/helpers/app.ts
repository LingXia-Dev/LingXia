import type { LxAppDriver } from 'lingxia-types/automation';

export const SHOWCASE_APP_ID = 'lingxia-showcase';

export function showcaseApp(): LxAppDriver {
  return lx.automation().lxapp(SHOWCASE_APP_ID);
}
