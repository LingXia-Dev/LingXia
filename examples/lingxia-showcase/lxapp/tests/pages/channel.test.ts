import { expect, test } from '@rongjs/test';
import type { LxAppDriver } from 'lingxia-types';

async function waitForText(
  app: LxAppDriver,
  css: string,
  predicate: (text: string) => boolean,
  timeoutMs = 10_000,
): Promise<string> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const element = await app.page.query({ page: 'channel', css, full: true });
    if (element.exists && predicate(element.text)) return element.text;
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw new Error(`Timed out waiting for channel ${css}`);
}

test('receives channel ticks, switches symbols, and reconnects', async () => {
  const app = lx.automation().lxapp();
  await app.nav.relaunch({ page: 'channel' });
  await app.page.waitFor({ page: 'channel', css: '[data-testid="channel-page"]' });

  expect(await waitForText(app, '[data-testid="channel-status"]', (text) => text === 'Connected'))
    .toBe('Connected');
  expect(await waitForText(app, '[data-testid="channel-price"]', (text) => text.startsWith('$')))
    .toContain('$');

  await app.page.click({ page: 'channel', css: '[data-testid="channel-symbol"][data-symbol="MSFT"]' });
  expect(await waitForText(app, '[data-testid="channel-active"]', (text) => text === 'MSFT'))
    .toBe('MSFT');
  expect(await waitForText(app, '[data-testid="channel-price"]', (text) => text.startsWith('$')))
    .toContain('$');

  await app.page.click({ page: 'channel', css: '[data-testid="channel-disconnect"]' });
  expect(await waitForText(app, '[data-testid="channel-status"]', (text) => text === 'Disconnected'))
    .toBe('Disconnected');
  await app.page.click({ page: 'channel', css: '[data-testid="channel-reconnect"]' });
  expect(await waitForText(app, '[data-testid="channel-status"]', (text) => text === 'Connected'))
    .toBe('Connected');
});
