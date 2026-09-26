import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { expect, spec } from '@lingxia/test';
import {
  waitForElementAttribute,
  waitForElementEnabled,
  waitForElementText,
} from '../helpers/page.js';

spec('streams a complete response from real page input', { app: SHOWCASE_APP_ID }, async (t) => {
  const app = t.app;
  await app.nav.relaunch({ page: 'stream' });
  await app.view.testId('stream-page', { page: 'stream' }).waitFor({ timeout: 30_000 });

  const prompt = `gate stream ${Date.now()}`;
  await app.view.testId("stream-input", { page: 'stream' }).fill(prompt);
  await waitForElementAttribute(
    t,
    'stream',
    '[data-testid="stream-input"]',
    'data-controlled-value',
    prompt,
  );
  await waitForElementEnabled(t, 'stream', '[data-testid="stream-send"]');
  await app.view.testId("stream-send", { page: 'stream' }).click();

  expect(await waitForElementText(
    t,
    'stream',
    '[data-testid="stream-message"][data-role="user"]',
    (text) => text.includes(prompt),
    15_000,
  )).toContain(prompt);
  await app.view.testId('stream-live', { page: 'stream' }).waitFor({ timeout: 30_000 });
  await app.view.testId('stream-live', { page: 'stream' }).waitFor({ state: 'detached', timeout: 20_000 });

  const response = await waitForElementText(
    t,
    'stream',
    '[data-testid="stream-message"][data-role="assistant"]',
    (text) => text.trim().length > 10,
    15_000,
  );
  expect(response.trim().length).toBeGreaterThan(10);
});
