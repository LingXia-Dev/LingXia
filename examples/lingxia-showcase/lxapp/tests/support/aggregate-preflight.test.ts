import { expect, test } from '@rongjs/test';
import { contract } from './contract.js';

const args = test.args as Record<string, string>;

contract({
  id: 'TARGET-001',
  title: 'match the aggregate entry to the running platform and framework',
  covers: ['aggregate target identity'],
  layer: 'host',
  levels: ['semantic'],
  scope: 'target',
  expectedOutcome: 'supported',
}, async ({ app }) => {
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
  expect(pages.length > 0).toBeTruthy();
  expect(pages.every(({ path }) => path.toLocaleLowerCase().endsWith(expectedExtension)))
    .toBeTruthy();
});
