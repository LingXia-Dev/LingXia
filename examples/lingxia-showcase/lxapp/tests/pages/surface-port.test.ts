import { expect, spec } from '@lingxia/test';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import {
  currentPageOrNull,
  waitForCurrentPage,
  waitForCurrentPageVisible,
  waitForElementAttribute,
  waitForElementText,
} from '../helpers/page.js';
import { bindFixture, eventually } from '../helpers/poll.js';

const MESSAGE_INPUT = 'input[placeholder="Message to parent page"]';

spec('exchange messages over the port navigateTo returns', {
  id: 'NAV-PORT-001',
  covers: ['lx.navigateTo', 'PageMessagePort.onMessage', 'PageMessagePort.postMessage'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app, namespace, defer } = bindFixture(t, 'NAV-PORT-001');
  const stateKey = `__lingxiaNavPort_${namespace.replace(/-/g, '_')}`;
  const outbound = `port-up-${namespace}`;

  const current = await currentPageOrNull(app);
  if (current?.name !== 'home') await app.nav.relaunch({ page: 'home' });
  await waitForCurrentPageVisible(app, 'home', '[data-testid="home-page"]');
  defer(async () => {
    await app.eval({
      script: `
        const state = globalThis[${JSON.stringify(stateKey)}];
        if (state?.off) state.off();
        delete globalThis[${JSON.stringify(stateKey)}];
      `,
    }).catch(() => undefined);
    const active = await currentPageOrNull(app);
    if (active?.name !== 'home') await app.nav.relaunch({ page: 'home' });
    await waitForCurrentPageVisible(app, 'home', '[data-testid="home-page"]');
  });

  // The opener keeps the port; the pushed page sees it as `this.opener`.
  // Navigation must not be awaited inside the eval that starts it: the
  // promise settles only after the new page is ready, which this eval blocks.
  await app.eval({
    script: `
      const state = { port: null, messages: [], off: null, error: null };
      globalThis[${JSON.stringify(stateKey)}] = state;
      lx.navigateTo({ page: 'surface', query: { fixture: ${JSON.stringify(namespace)} } })
        .then((port) => {
          state.port = port;
          state.off = port.onMessage((message) => state.messages.push(message));
        })
        .catch((error) => { state.error = String(error && error.message || error); });
      return 'scheduled';
    `,
  });
  await waitForCurrentPage(app, 'surface');
  await app.page.waitFor({ page: 'surface', css: '[data-testid="surface-page"]', state: 'visible' });
  const port = await eventually(
    () => app.eval({
      script: `
        const state = globalThis[${JSON.stringify(stateKey)}];
        return {
          error: state?.error ?? null,
          shape: state?.port ? typeof state.port.postMessage === 'function' && typeof state.port.onMessage === 'function' : null,
        };
      `,
    }) as Promise<{ error: string | null; shape: boolean | null }>,
    (value) => value.error !== null || value.shape !== null,
    { describe: 'navigateTo to settle with a message port', timeoutMs: 10_000 },
  );
  expect(port.error).toBe(null);
  expect(port.shape).toBe(true);

  await t.step('opener → page', async () => {
    await app.eval({
      script: `globalThis[${JSON.stringify(stateKey)}].port.postMessage({ ping: ${JSON.stringify(namespace)} });`,
    });
    const inbound = await waitForElementText(
      app,
      'surface',
      '[data-testid="surface-inbound"]',
      (text) => text.includes(namespace),
    );
    expect(JSON.parse(inbound)).toEqual({ ping: namespace });
    expect(await waitForElementText(
      app,
      'surface',
      '[data-testid="surface-inbound-count"]',
      (text) => text.trim() === '1',
    )).toBe('1');
  });

  await t.step('page → opener, then the page pops itself', async () => {
    await app.page.fill({ page: 'surface', css: MESSAGE_INPUT, text: outbound });
    await waitForElementAttribute(app, 'surface', MESSAGE_INPUT, 'data-controlled-value', outbound);
    await app.page.click({ page: 'surface', css: '[data-testid="surface-send-message"]' });

    const messages = await eventually(
      () => app.eval({
        script: `return globalThis[${JSON.stringify(stateKey)}]?.messages ?? []`,
      }) as Promise<Array<{ message?: string; timestamp?: number }>>,
      (value) => value.some((message) => message.message === outbound),
      { describe: 'page message delivered to the opener port', timeoutMs: 10_000 },
    );
    expect(typeof messages.find((message) => message.message === outbound)?.timestamp).toBe('number');

    await waitForCurrentPageVisible(app, 'home', '[data-testid="home-page"]');
    expect((await app.nav.stack()).map((page) => page.name)).toEqual(['home']);
  });
});
