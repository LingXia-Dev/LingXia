import { expect, test } from '@rongjs/test';
import { LX_RUNTIME_SURFACES } from './manifest.js';

for (const surface of LX_RUNTIME_SURFACES) {
  test(`publishes every ${surface.name} member`, async () => {
    const members = JSON.stringify(surface.members);
    const result = await lx.automation().lxapp().eval({
      script: `
        const target = ${surface.expression};
        return {
          available: target !== null && typeof target !== 'undefined',
          missing: ${members}.filter((name) => typeof target[name] === 'undefined'),
        };
      `,
    }) as { available: boolean; missing: string[] };

    expect(result.available).toBeTruthy();
    expect(result.missing).toEqual([]);
  });
}
