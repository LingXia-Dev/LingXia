import { expect, test } from '@lingxia/test';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import {
  currentPageOrNull,
  waitForCurrentPage,
  waitForCurrentPageVisible,
  waitForElementAttribute,
  waitForElementText,
} from '../helpers/page.js';
import { bindFixture, eventually } from '../helpers/poll.js';
import type { PageHandle } from '@lingxia/types';

/** What the probe keeps on Logic's `globalThis` under its spec's key. */
interface PortState {
  port: PageHandle | null;
  messages: Array<{ message?: string; timestamp?: number }>;
  off: (() => void) | null;
  error: string | null;
}
type PortStates = Record<string, PortState | undefined>;

const MESSAGE_INPUT = 'input[placeholder="Message to parent page"]';

test('exchange messages over the port navigateTo returns', {
  id: 'NAV-PORT-001',
  covers: ['lx.nav.go', 'PageHandle.onMessage', 'PageHandle.send'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app, namespace, defer } = bindFixture(t, 'NAV-PORT-001');
  const stateKey = `__lingxiaNavPort_${namespace.replace(/-/g, '_')}`;
  const outbound = `port-up-${namespace}`;

  const current = await currentPageOrNull(app);
  if (current?.name !== 'home') await app.nav.reset({ page: 'home' });
  await waitForCurrentPageVisible(app, 'home', '[data-testid="home-page"]');
  defer(async () => {
    await app.logic.eval((_, key) => {
      const states = globalThis as unknown as PortStates;
      states[key]?.off?.();
      delete states[key];
    }, stateKey).catch(() => undefined);
    const active = await currentPageOrNull(app);
    if (active?.name !== 'home') await app.nav.reset({ page: 'home' });
    await waitForCurrentPageVisible(app, 'home', '[data-testid="home-page"]');
  });

  // The opener keeps the handle; the pushed page sees its end as `page.opener`.
  // `opened` must not be awaited inside the eval that starts it: it settles
  // only after the new page is ready, which this eval blocks.
  await app.logic.eval(({ lx }, key, fixture) => {
    const state: PortState = { port: null, messages: [], off: null, error: null };
    (globalThis as unknown as PortStates)[key] = state;
    const handle = lx.nav.go({ page: 'surface', params: { fixture } });
    state.port = handle;
    state.off = handle.onMessage((message) => {
      state.messages.push(message as PortState['messages'][number]);
    });
    handle.opened.catch((error: { message?: string } | null) => { state.error = String(error?.message ?? error); });
    return 'scheduled';
  }, stateKey, namespace);
  // Desktop only preloads the landing tab. This spec is the first visit to
  // `surface`, and Windows CI has taken >10s to create that WebView and fire
  // onReady (the page was already current). Match the other cold-nav specs.
  await waitForCurrentPage(app, 'surface', 30_000);
  // Bind the pushed instance (the current page): another `surface` may be open as a window.
  const surface = (await app.page()).view;
  await surface.testId('surface-page').waitFor({ state: 'visible', timeout: 30_000 });
  const port = await eventually(
    () => app.logic.eval((_, key) => {
      const state = (globalThis as unknown as PortStates)[key];
      return {
        error: state?.error ?? null,
        shape: state?.port ? typeof state.port.send === 'function' && typeof state.port.onMessage === 'function' : null,
      };
    }, stateKey),
    (value) => value.error !== null || value.shape !== null,
    { describe: 'navigateTo to settle with a message port', timeoutMs: 10_000 },
  );
  expect(port.error).toBe(null);
  expect(port.shape).toBe(true);

  await t.step('opener → page', async () => {
    await app.logic.eval((_, key, ping) => {
      (globalThis as unknown as PortStates)[key]?.port?.send({ ping });
    }, stateKey, namespace);
    const inbound = await waitForElementText(
      t,
      surface,
      '[data-testid="surface-inbound"]',
      (text) => text.includes(namespace),
    );
    expect(JSON.parse(inbound)).toEqual({ ping: namespace });
    expect(await waitForElementText(
      t,
      surface,
      '[data-testid="surface-inbound-count"]',
      (text) => text.trim() === '1',
    )).toBe('1');
  });

  await t.step('page → opener, then the page pops itself', async () => {
    await surface.css(MESSAGE_INPUT).fill(outbound);
    await waitForElementAttribute(surface, MESSAGE_INPUT, 'data-controlled-value', outbound);
    await surface.testId("surface-send-message").click();

    const messages = await eventually(
      () => app.logic.eval((_, key) => (globalThis as unknown as PortStates)[key]?.messages ?? [], stateKey),
      (value) => value.some((message) => message.message === outbound),
      { describe: 'page message delivered to the opener port', timeoutMs: 10_000 },
    );
    expect(typeof messages.find((message) => message.message === outbound)?.timestamp).toBe('number');

    await waitForCurrentPageVisible(app, 'home', '[data-testid="home-page"]');
    expect((await app.nav.stack()).map((page) => page.name)).toEqual(['home']);
  });
});
