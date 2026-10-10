import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { expect, spec } from '@lingxia/test';
import type { PageContract } from '@lingxia/types/page';
import type { Message } from '../../pages/stream/index.js';
import { bindFixture } from '../helpers/poll.js';
import {
  waitForElementAttribute,
  waitForElementEnabled,
  waitForElementText,
} from '../helpers/page.js';

spec('streams a complete response from real page input', { app: SHOWCASE_APP_ID }, async (t) => {
  const app = t.app;
  await app.nav.relaunch({ page: 'stream' });
  const stream = (await app.page({ name: 'stream' }, { timeout: 30_000 })).view;
  await stream.testId('stream-page').waitFor({ timeout: 30_000 });

  const prompt = `gate stream ${Date.now()}`;
  await stream.testId("stream-input").fill(prompt);
  await waitForElementAttribute(
    stream,
    '[data-testid="stream-input"]',
    'data-controlled-value',
    prompt,
  );
  await waitForElementEnabled(stream, '[data-testid="stream-send"]');
  await stream.testId("stream-send").click();

  expect(await waitForElementText(
    t,
    stream,
    '[data-testid="stream-message"][data-role="user"]',
    (text) => text.includes(prompt),
    15_000,
  )).toContain(prompt);
  await stream.testId('stream-live').waitFor({ timeout: 30_000 });
  await stream.testId('stream-live').waitFor({ state: 'detached', timeout: 20_000 });

  const response = await waitForElementText(
    t,
    stream,
    '[data-testid="stream-message"][data-role="assistant"]',
    (text) => text.trim().length > 10,
    15_000,
  );
  expect(response.trim().length).toBeGreaterThan(10);
});

spec('cancel an active response, finish Logic cleanup, and send again', {
  id: 'STREAM-CANCEL-001',
  app: SHOWCASE_APP_ID,
  covers: ['lx.navigateTo'],
  start: { page: 'stream' },
}, async (t) => {
  const { app, namespace, defer } = bindFixture(t, 'STREAM-CANCEL-001');
  const page = await app.page<PageContract<{ messages: Message[]; isStreaming: boolean }>>({ name: 'stream' });
  defer(async () => { await app.nav.relaunch({ page: 'home' }); });
  await app.clock.install();

  for (let turn = 0; turn < 2; turn++) {
    const prompt = `${namespace} turn ${turn}`;
    await page.view.testId('stream-input').fill(prompt);
    await expect(page.view.testId('stream-input')).toHaveAttribute('data-controlled-value', prompt);
    await page.view.testId('stream-send').click();
    await expect.poll(async () => (await page.data()).isStreaming).toBe(true);
    // Freeze after the first real chunk. Natural completion must not be able
    // to satisfy the cancellation assertions while we wait for bridge traffic.
    let partial = '';
    for (let tick = 0; tick < 20 && !/[a-z]/i.test(partial); tick++) {
      await app.clock.tick(50);
      partial = await page.view.testId('stream-live').textContent();
    }
    expect(/[a-z]/i.test(partial)).toBe(true);
    await page.view.testId('stream-stop').click();
    await expect(page.view.testId('stream-live')).not.toBeAttached();
    // Generator return waits for its pending next(): release that one word,
    // without letting an uncancelled response reach normal completion.
    await app.clock.tick(90);
    await expect.poll(async () => (await page.data()).isStreaming).toBe(false);
    const { messages } = await page.data();
    // Only the generator's finally block commits this partial response.
    expect(messages.length).toBe((turn + 1) * 2);
    expect(messages[turn * 2].content).toBe(prompt);
    expect(messages[turn * 2 + 1].role).toBe('assistant');
    expect(messages[turn * 2 + 1].content.trim().length).toBeGreaterThan(0);
    expect(messages[turn * 2 + 1].content).toContain(partial.trim());
    await expect(page.view.testId('stream-input')).toBeEnabled();
  }
});
