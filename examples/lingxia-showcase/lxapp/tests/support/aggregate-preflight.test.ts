import { expect, spec } from '@lingxia/test';
import { bindFixture, specNamespace } from '../helpers/poll.js';
import { SHOWCASE_APP_ID } from '../helpers/app.js';

const args = globalThis.__LINGXIA_AUTOMATION_HOST__?.args ?? {} as Record<string, string>;

spec("match the aggregate entry to the running platform and framework", { id: "TARGET-001", covers: ['lx.app.getBaseInfo', 'LxAppDriver.pages'], app: SHOWCASE_APP_ID }, async (t) => {
  const { app } = bindFixture(t, "TARGET-001");

  const expectedPlatform = args.platform?.toLocaleLowerCase();
  const expectedFramework = args.framework?.toLocaleLowerCase();
  if (!expectedPlatform || !expectedFramework) {
    throw new Error('Aggregate entries require --arg platform=<name> and --arg framework=react|vue');
  }
  if (!['react', 'vue'].includes(expectedFramework)) {
    throw new Error(`Unsupported aggregate framework '${expectedFramework}'`);
  }

  const actualPlatform = await app.eval({
    script: 'return String(lx.app.getBaseInfo().os || "").toLowerCase()',
  });
  expect(actualPlatform).toBe(expectedPlatform);

  const expectedExtension = expectedFramework === 'react' ? '.tsx' : '.vue';
  const pages = await app.pages();
  expect(pages.length).toBeGreaterThan(0);
  expect(pages.every(({ path }) => path.toLocaleLowerCase().endsWith(expectedExtension)))
    .toBeTruthy();
});
