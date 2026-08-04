import type { LxAppDriver } from 'lingxia-types/automation';

export async function runtimePlatform(app: LxAppDriver): Promise<string> {
  return app.eval({
    script: 'return String(lx.app.getBaseInfo().os || "").toLowerCase()',
  }) as Promise<string>;
}
