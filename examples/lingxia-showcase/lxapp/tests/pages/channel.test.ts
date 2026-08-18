import { expect, spec } from '@lingxia/test';
import { showcaseApp } from '../helpers/app.js';
import { waitForElementText } from '../helpers/page.js';

const waitForText = (
  app: Parameters<typeof waitForElementText>[0],
  css: string,
  predicate: (text: string) => boolean,
) => waitForElementText(app, 'channel', css, predicate, 30_000);

spec('receives channel ticks, switches symbols, and reconnects', async () => {
  const app = showcaseApp();
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
