import { expect, test } from '@rongjs/test';
import { showcaseApp } from '../helpers/app.js';
import { waitForElementAttribute } from '../helpers/page.js';
import { waitForCurrentPage } from '../helpers/page.js';
import { contract, eventually } from '../support/contract.js';

contract({
  id: 'UI-NAV-001',
  title: 'run navigation APIs from the rendered UI controls',
  covers: ['lx.navigateTo', 'lx.navigateBack', 'lx.redirectTo', 'lx.switchTab'],
  layer: 'logic',
  levels: ['semantic', 'boundary', 'lifecycle'],
  scope: 'portable',
  expectedOutcome: 'supported',
}, async ({ app }) => {
  await app.nav.relaunch({ page: 'ui', query: { type: 'navigation' } });
  await app.page.waitFor({ page: 'ui', css: '[data-testid="ui-navigate-to"]', state: 'visible' });

  await app.page.click({ page: 'ui', css: '[data-testid="ui-navigate-to"]' });
  await eventually(() => app.nav.stack(), (stack) => stack.length === 2, {
    describe: 'UI navigateTo to push a second page instance',
  });

  // The push created a fresh instance of this same route; wait for its
  // document before driving the next control.
  await app.page.waitFor({ page: 'ui', css: '[data-testid="ui-navigate-back"]', state: 'visible' });
  await app.page.click({ page: 'ui', css: '[data-testid="ui-navigate-back"]' });
  await eventually(() => app.nav.stack(), (stack) => stack.length === 1, {
    describe: 'UI navigateBack to pop the page instance',
  });

  await app.page.click({ page: 'ui', css: '[data-testid="ui-redirect-to"]' });
  await eventually(() => app.nav.stack(), (stack) => stack.length === 1 && stack[0]?.name === 'ui', {
    describe: 'UI redirectTo to replace the current page',
  });

  await app.page.click({ page: 'ui', css: '[data-testid="ui-switch-tab"]' });
  await waitForCurrentPage(app, 'home');
  expect((await app.nav.stack()).map(({ name }) => name)).toEqual(['home']);
});

test('rejects invalid native-surface dimensions before opening a host surface', async () => {
  const app = showcaseApp();
  await app.nav.relaunch({ page: 'ui', query: { type: 'surface' } });
  await app.page.waitFor({ page: 'ui', css: '[data-testid="open-surface"]' });

  await app.page.fill({ page: 'ui', css: 'input[placeholder="width (px or %)"]', text: 'invalid' });
  await app.page.fill({ page: 'ui', css: 'input[placeholder="height (px or %)"]', text: '50%' });
  await waitForElementAttribute(app, 'ui', '[data-testid="open-surface"]', 'data-surface-width', 'invalid');
  await waitForElementAttribute(app, 'ui', '[data-testid="open-surface"]', 'data-surface-height', '50%');
  await app.page.click({ page: 'ui', css: '[data-testid="open-surface"]' });
  await app.page.waitFor({ page: 'ui', css: '[data-testid="size-error"]' });

  const error = await app.page.query({ page: 'ui', css: '[data-testid="size-error"]', full: true });
  expect(error.exists).toBeTruthy();
  expect(error.exists && error.text.trim().length > 0).toBeTruthy();
});
