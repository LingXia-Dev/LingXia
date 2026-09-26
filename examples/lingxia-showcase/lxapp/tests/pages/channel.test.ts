import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { expect, spec, type Fixture } from '@lingxia/test';
import { waitForElementText } from '../helpers/page.js';

const waitForText = (
  t: Fixture,
  css: string,
  predicate: (text: string) => boolean,
) => waitForElementText(t, 'channel', css, predicate, 30_000);

spec('receives channel ticks, switches symbols, and reconnects', { app: SHOWCASE_APP_ID }, async (t) => {
  const app = t.app;
  await app.nav.relaunch({ page: 'channel' });
  await app.view.testId('channel-page', { page: 'channel' }).waitFor({ timeout: 30_000 });

  expect(await waitForText(t, '[data-testid="channel-status"]', (text) => text === 'Connected'))
    .toBe('Connected');
  expect(await waitForText(t, '[data-testid="channel-price"]', (text) => text.startsWith('$')))
    .toContain('$');

  await app.view.css('[data-testid="channel-symbol"][data-symbol="MSFT"]', { page: 'channel' }).click();
  expect(await waitForText(t, '[data-testid="channel-active"]', (text) => text === 'MSFT'))
    .toBe('MSFT');
  expect(await waitForText(t, '[data-testid="channel-price"]', (text) => text.startsWith('$')))
    .toContain('$');

  await app.view.testId("channel-disconnect", { page: 'channel' }).click();
  expect(await waitForText(t, '[data-testid="channel-status"]', (text) => text === 'Disconnected'))
    .toBe('Disconnected');
  await app.view.testId("channel-reconnect", { page: 'channel' }).click();
  expect(await waitForText(t, '[data-testid="channel-status"]', (text) => text === 'Connected'))
    .toBe('Connected');
});
