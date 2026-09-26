import type { AppDriver } from './app.js';


export async function runtimePlatform(app: AppDriver): Promise<string> {
  return app.eval({
    script: 'return String(lx.host.getBaseInfo().os || "").toLowerCase()',
  }) as Promise<string>;
}
