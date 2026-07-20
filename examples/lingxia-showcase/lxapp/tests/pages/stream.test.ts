import { expect, test } from '@rongjs/test';
import type { LxAppDriver } from 'lingxia-types';

async function waitForElementText(
  app: LxAppDriver,
  page: string,
  css: string,
  predicate: (text: string) => boolean,
  timeoutMs = 15_000,
): Promise<string> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const element = await app.page.query({ page, css, full: true });
    if (element.exists && predicate(element.text)) return element.text;
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw new Error(`Timed out waiting for ${page} ${css}`);
}

test('streams a complete response from real page input', async () => {
  const app = lx.automation().lxapp();
  await app.nav.relaunch({ page: 'stream' });
  await app.page.waitFor({ page: 'stream', css: '[data-testid="stream-page"]' });

  const prompt = `gate stream ${Date.now()}`;
  await app.page.fill({ page: 'stream', css: '[data-testid="stream-input"]', text: prompt });
  await app.page.click({ page: 'stream', css: '[data-testid="stream-send"]' });

  expect(await waitForElementText(
    app,
    'stream',
    '[data-testid="stream-message"][data-role="user"]',
    (text) => text.includes(prompt),
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
  );
  expect(response.trim().length > 10).toBeTruthy();
});
