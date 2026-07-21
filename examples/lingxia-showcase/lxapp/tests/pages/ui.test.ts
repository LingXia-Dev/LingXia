import { expect, test } from '@rongjs/test';
import { waitForElementAttribute } from '../helpers/page.js';

test('rejects invalid native-surface dimensions before opening a host surface', async () => {
  const app = lx.automation().lxapp();
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
