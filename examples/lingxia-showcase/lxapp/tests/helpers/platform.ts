import type { TestApp } from '@lingxia/test';

export async function runtimePlatform(app: TestApp): Promise<string> {
  return app.logic.eval(({ lx }) => String(lx.host.getBaseInfo().os || '').toLowerCase());
}
