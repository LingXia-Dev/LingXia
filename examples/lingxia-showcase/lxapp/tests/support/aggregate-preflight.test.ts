import { expect, spec } from '@lingxia/test';
import { bindFixture, specNamespace } from '../helpers/poll.js';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { runtimePlatform } from '../helpers/platform.js';

const args = globalThis.__LINGXIA_AUTOMATION_HOST__?.args ?? {} as Record<string, string>;

spec("match the aggregate entry to the running platform", { id: "TARGET-001", covers: ['lx.host.getBaseInfo', 'LxAppDriver.pages'], app: SHOWCASE_APP_ID }, async (t) => {
  const { app } = bindFixture(t, "TARGET-001");

  const expectedPlatform = args.platform?.toLocaleLowerCase();
  if (!expectedPlatform) {
    throw new Error('Aggregate entries require --arg platform=<name>');
  }

  const actualPlatform = await runtimePlatform(app);
  expect(actualPlatform).toBe(expectedPlatform);

  const pages = await app.pages();
  expect(pages.length).toBeGreaterThan(0);
  expect(pages.every(({ path }) => path.toLocaleLowerCase().endsWith('.tsx')))
    .toBeTruthy();
});
