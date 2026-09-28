import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { expect, spec } from '@lingxia/test';
import { waitForElementText } from '../helpers/page.js';

spec('receives channel ticks, switches symbols, and reconnects', { app: SHOWCASE_APP_ID }, async (t) => {
  const app = t.app;
  await app.nav.relaunch({ page: 'channel' });
  const channel = (await app.page({ name: 'channel' }, { timeout: 30_000 })).view;
  await channel.testId('channel-page').waitFor({ timeout: 30_000 });
  const waitForText = (css: string, predicate: (text: string) => boolean) =>
    waitForElementText(t, channel, css, predicate, 30_000);

  expect(await waitForText('[data-testid="channel-status"]', (text) => text === 'Connected'))
    .toBe('Connected');
  expect(await waitForText('[data-testid="channel-price"]', (text) => text.startsWith('$')))
    .toContain('$');

  await channel.css('[data-testid="channel-symbol"][data-symbol="MSFT"]').click();
  expect(await waitForText('[data-testid="channel-active"]', (text) => text === 'MSFT'))
    .toBe('MSFT');
  expect(await waitForText('[data-testid="channel-price"]', (text) => text.startsWith('$')))
    .toContain('$');

  await channel.testId("channel-disconnect").click();
  expect(await waitForText('[data-testid="channel-status"]', (text) => text === 'Disconnected'))
    .toBe('Disconnected');
  await channel.testId("channel-reconnect").click();
  expect(await waitForText('[data-testid="channel-status"]', (text) => text === 'Connected'))
    .toBe('Connected');
});
