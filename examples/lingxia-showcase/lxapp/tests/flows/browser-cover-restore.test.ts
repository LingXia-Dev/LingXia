import { expect, test } from '@rongjs/test';

// Regression: closing presented web tabs must restore the home lxapp as a
// rendered page, never a blank loading state.
test('restores rendered home content after closing covering web tabs', async () => {
  const automation = lx.automation();
  const app = automation.lxapp();
  const browser = automation.browser;

  async function waitFor<T>(
    read: () => Promise<T | undefined>,
    label: string,
    timeoutMs = 15_000,
  ): Promise<T> {
    const deadline = Date.now() + timeoutMs;
    let last: unknown;
    while (Date.now() < deadline) {
      try {
        const value = await read();
        if (value !== undefined) return value;
      } catch (error) {
        last = error;
      }
      await new Promise((resolve) => setTimeout(resolve, 200));
    }
    throw new Error(`${label} not observed within ${timeoutMs}ms; last error: ${String(last)}`);
  }

  const renderedBodyLength = async (): Promise<number> => {
    const result = await app.page.eval({
      script: 'return document.body ? document.body.innerText.length : -1',
    });
    return Number(result);
  };

  // Baseline: the home lxapp has a live rendered page before any cover.
  const before = await waitFor(async () => {
    const length = await renderedBodyLength();
    return length > 0 ? length : undefined;
  }, 'baseline home page renders');
  expect(before > 0).toBeTruthy();

  const tabsBefore = new Set((await browser.tabs()).map((tab) => tab.tab_id));

  await app.eval({
    timeoutMs: 20_000,
    script: `await lx.openSurface({ url: 'lingxia://settings' });`,
  });
  await app.eval({
    timeoutMs: 20_000,
    script: `await lx.openSurface({ url: 'lingxia://downloads' });`,
  });

  const opened = await waitFor(async () => {
    const fresh = (await browser.tabs()).filter((tab) => !tabsBefore.has(tab.tab_id));
    return fresh.length >= 2 ? fresh : undefined;
  }, 'two covering web tabs open');

  // Close them one by one, mirroring repeated user close actions.
  for (const tab of opened) {
    await browser.close({ tab: tab.tab_id });
    await new Promise((resolve) => setTimeout(resolve, 400));
  }

  await waitFor(async () => {
    const remaining = (await browser.tabs()).filter((tab) => !tabsBefore.has(tab.tab_id));
    return remaining.length === 0 ? true : undefined;
  }, 'covering web tabs closed');

  // Graph converges back to the root main.
  const layout = await waitFor(async () => {
    const snapshot = await app.surfaceLayout();
    return snapshot.activeMainId === snapshot.mainSwitcher.rootSurfaceId
      ? snapshot
      : undefined;
  }, 'root main active after closes');
  expect(layout.mains.includes('lingxia-showcase')).toBeTruthy();

  // The restored page must be physically live again: a dead/spinner page has
  // no rendered body text.
  const after = await waitFor(async () => {
    const length = await renderedBodyLength();
    return length > 0 ? length : undefined;
  }, 'restored home page has rendered body text', 20_000);
  expect(after > 0).toBeTruthy();
});
