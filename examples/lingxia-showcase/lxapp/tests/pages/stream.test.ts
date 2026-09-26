import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { expect, spec } from '@lingxia/test';
import {
  waitForElementAttribute,
  waitForElementEnabled,
  waitForElementText,
} from '../helpers/page.js';

spec('streams a complete response from real page input', async (t) => {
  const app = t.apps.lxapp(SHOWCASE_APP_ID);
  await app.nav.relaunch({ page: 'stream' });
  await app.page.waitFor({ page: 'stream', css: '[data-testid="stream-page"]' });

  const prompt = `gate stream ${Date.now()}`;
  await app.view.testId("stream-input", { page: 'stream' }).fill(prompt);
  await waitForElementAttribute(
    app,
    'stream',
    '[data-testid="stream-input"]',
    'data-controlled-value',
    prompt,
  );
  await waitForElementEnabled(app, 'stream', '[data-testid="stream-send"]');
  await app.view.testId("stream-send", { page: 'stream' }).click();

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
  expect(response.trim().length).toBeGreaterThan(10);
});
