import { SHOWCASE_APP_ID, rawApp } from '../helpers/app.js';
import { expect, spec } from '@lingxia/test';
import { waitForElementText } from '../helpers/page.js';

// String scripts and raw page reads go to the raw driver; see `rawApp`.
const raw = rawApp();

const waitForText = (
  app: Parameters<typeof waitForElementText>[0],
  css: string,
  predicate: (text: string) => boolean,
) => waitForElementText(app, 'channel', css, predicate, 30_000);

spec('receives channel ticks, switches symbols, and reconnects', async (t) => {
  const app = t.apps.lxapp(SHOWCASE_APP_ID);
  await app.nav.relaunch({ page: 'channel' });
  await raw.page.waitFor({ page: 'channel', css: '[data-testid="channel-page"]' });

  expect(await waitForText(raw, '[data-testid="channel-status"]', (text) => text === 'Connected'))
    .toBe('Connected');
  expect(await waitForText(raw, '[data-testid="channel-price"]', (text) => text.startsWith('$')))
    .toContain('$');

  await app.view.css('[data-testid="channel-symbol"][data-symbol="MSFT"]', { page: 'channel' }).click();
  expect(await waitForText(raw, '[data-testid="channel-active"]', (text) => text === 'MSFT'))
    .toBe('MSFT');
  expect(await waitForText(raw, '[data-testid="channel-price"]', (text) => text.startsWith('$')))
    .toContain('$');

  await app.view.testId("channel-disconnect", { page: 'channel' }).click();
  expect(await waitForText(raw, '[data-testid="channel-status"]', (text) => text === 'Disconnected'))
    .toBe('Disconnected');
  await app.view.testId("channel-reconnect", { page: 'channel' }).click();
  expect(await waitForText(raw, '[data-testid="channel-status"]', (text) => text === 'Connected'))
    .toBe('Connected');
});
