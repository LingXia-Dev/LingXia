import { expect, spec } from '@lingxia/test';
import { showcaseApp } from '../helpers/app.js';
import {
  waitForElementAttribute,
  waitForElementEnabled,
  waitForElementText,
} from '../helpers/page.js';

spec('streams a complete response from real page input', async () => {
  const app = showcaseApp();
  await app.nav.relaunch({ page: 'stream' });
  await app.page.waitFor({ page: 'stream', css: '[data-testid="stream-page"]' });

  const prompt = `gate stream ${Date.now()}`;
  await app.page.fill({ page: 'stream', css: '[data-testid="stream-input"]', text: prompt });
  await waitForElementAttribute(
    app,
    'stream',
    '[data-testid="stream-input"]',
    'data-controlled-value',
    prompt,
  );
  await waitForElementEnabled(app, 'stream', '[data-testid="stream-send"]');
  await app.page.click({ page: 'stream', css: '[data-testid="stream-send"]' });

  expect(await waitForElementText(
    app,
    'stream',
    '[data-testid="stream-message"][data-role="user"]',
    (text) => text.includes(prompt),
    15_000,
  )).toContain(prompt);
  await app.page.waitFor({ page: 'stream', css: '[data-testid="stream-live"]' });
  await app.page.waitFor({
    page: 'stream',
    css: '[data-testid="stream-live"]',
    state: 'gone',
    timeoutMs: 20_000,
  });

  const response = await waitForElementText(
    app,
    'stream',
    '[data-testid="stream-message"][data-role="assistant"]',
    (text) => text.trim().length > 10,
    15_000,
  );
  expect(response.trim().length > 10).toBeTruthy();
});
