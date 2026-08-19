import { expect, spec } from '@lingxia/test';
import { showcaseApp } from '../helpers/app.js';
import {
  waitForElementAttribute,
  waitForElementEnabled,
  waitForElementText,
} from '../helpers/page.js';

spec('greets through real page input and the Logic bridge', async () => {
  const app = showcaseApp();
  await app.nav.relaunch({ page: 'home' });
  await app.page.waitFor({ page: 'home', css: '[data-testid="home-page"]' });

  const name = `Gate ${Date.now()}`;
  await app.page.fill({ page: 'home', css: '[data-testid="home-name"]', text: name });
  await waitForElementAttribute(
    app,
    'home',
    '[data-testid="home-name"]',
    'data-controlled-value',
    name,
  );
  await waitForElementEnabled(app, 'home', '[data-testid="home-greet"]');
  await app.page.click({ page: 'home', css: '[data-testid="home-greet"]' });

  expect(await waitForElementText(
    app,
    'home',
    '[data-testid="home-greeting"]',
    (text) => text.includes(name),
    30_000,
  )).toContain(name);
});
