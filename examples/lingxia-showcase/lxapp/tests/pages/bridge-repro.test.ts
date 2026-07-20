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
    const element = await app.page.query({ page: 'bridge-repro', css, full: true });
    if (element.exists && predicate(element.text)) return element.text;
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw new Error(`Timed out waiting for ${css}`);
}

test('keeps bootstrap, calls, and streams healthy across the page bridge', async () => {
  const app = lx.automation().lxapp();
  try {
    await app.nav.relaunch({ page: 'bridge-repro' });
    await app.page.waitFor({
      page: 'bridge-repro',
      css: '[data-testid="bridge-repro-page"][data-automation-contract="bridge-v1"]',
    });
    await app.page.waitFor({ page: 'bridge-repro', css: '#bootstrap-verdict' });

    expect(await waitForText(app, '#bootstrap-verdict', (text) => text.includes('PASS')))
      .toContain('PASS');

    await app.page.click({ page: 'bridge-repro', css: '#btn-echo' });
    expect(await waitForText(app, '#stat-echo', (text) => text.includes('echo #1 ok')))
      .toContain('echo #1 ok');

    await app.page.click({ page: 'bridge-repro', css: '#btn-restart' });
    await waitForText(app, '#stat-received', (text) => Number.parseInt(text.replace(/\D+/g, ''), 10) >= 2);
    expect(await waitForText(app, '#stream-verdict', (text) => text.includes('PASS')))
      .toContain('PASS');
    expect(await waitForText(app, '#stat-gaps', (text) => text.includes('none'))).toContain('none');
    expect(await waitForText(app, '#stat-error', (text) => text.includes('none'))).toContain('none');
    await app.page.click({ page: 'bridge-repro', css: '#btn-stop' });
    await new Promise((resolve) => setTimeout(resolve, 100));
  } catch (error) {
    try {
      const screenshot = await app.page.screenshot({ page: 'bridge-repro' });
      await test.attach?.('bridge-repro-failure.png', {
        mimeType: 'image/png',
        base64: screenshot.base64,
      });
    } catch {
      // Preserve the bridge failure when screenshot capture also fails.
    }
    throw error;
  } finally {
    try {
      await app.nav.relaunch({ page: 'home' });
    } catch {
      // Keep a cleanup failure from hiding the original bridge assertion.
    }
  }
});
