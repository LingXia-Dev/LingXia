import { expect, test } from '@lingxia/test';
import { bindFixture, specNamespace } from '@lingxia/test/showcase/helpers/poll.js';
import { SHOWCASE_APP_ID } from '@lingxia/test/showcase/helpers/app.js';
import { runtimePlatform } from '@lingxia/test/showcase/helpers/platform.js';

const args = globalThis.__LINGXIA_AUTOMATION_HOST__?.args ?? {} as Record<string, string>;

test("match the aggregate entry to the running platform", { id: "TARGET-001", covers: ['lx.host.info', 'LxAppDriver.pages'], app: SHOWCASE_APP_ID }, async (t) => {
  const { app } = bindFixture(t, "TARGET-001");

  const expectedPlatform = args.platform?.toLocaleLowerCase();
  if (!expectedPlatform) {
    throw new Error('Aggregate entries require --arg platform=<name>');
  }

  const actualPlatform = await runtimePlatform(app);
  expect(actualPlatform).toBe(expectedPlatform);

  const pages = await app.pages();
  expect(pages.length).toBeGreaterThan(0);
  // A GPU page's path names its built View bundle.
  expect(pages.every(({ path }) => path.toLocaleLowerCase().endsWith('/index.js')))
    .toBeTruthy();
});
