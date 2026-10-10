import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { expect, spec } from '@lingxia/test';
import { waitForElementText } from '../helpers/page.js';
import type { LingXiaBridgeInterface } from '@lingxia/bridge';

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

spec('closing a real channel ends reads and rejects subsequent sends', {
  id: 'CHANNEL-ERROR-001',
  covers: ['LxChannel.close', 'LxChannel.send', 'BRIDGE_STREAM_CLOSED', 'BRIDGE_TOPIC_NOT_FOUND'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  await t.app.nav.relaunch({ page: 'channel' });
  const view = (await t.app.page({ name: 'channel' })).view;
  await view.testId('channel-page').waitFor();
  const result = await view.eval(async ({ window }) => {
    const bridge = window.LingXiaBridge as LingXiaBridgeInterface;
    const channel = await bridge.raw.channel.open('tickerSession');
    const errors: string[] = [];
    const closes: Array<{ code: string; reason: string }> = [];
    // Consume the initial ticker snapshot before asking for a pending read.
    channel.on('data', () => {});
    channel.on('error', (error) => errors.push(String(error.code)));
    channel.on('close', (code, reason) => closes.push({ code: code ?? '', reason: reason ?? '' }));
    try {
      const iterator = channel[Symbol.asyncIterator]();
      const pending = iterator.next();
      channel.close('done', 'test finished');
      channel.close('duplicate', 'must be ignored');
      const ended = await pending;
      channel.send({ type: 'subscribe', symbol: 'MSFT' });
      const after = await iterator.next();
      let missing = '';
      try {
        const unexpected = await bridge.raw.channel.open('noSuchChannel');
        unexpected.close();
      } catch (error) {
        missing = (error as { code: string }).code;
      }
      return { closes, errors, ended: ended.done === true, after: after.done === true, missing };
    } finally {
      channel.close();
    }
  });
  expect(result.closes).toEqual([{ code: 'done', reason: 'test finished' }]);
  expect(result.errors).toEqual(['BRIDGE_STREAM_CLOSED']);
  expect(result.ended).toBe(true);
  expect(result.after).toBe(true);
  expect(result.missing).toBe('BRIDGE_TOPIC_NOT_FOUND');
});
