import { expect, spec } from '@lingxia/test';
import { showcaseApp } from '../helpers/app.js';
import { waitForCurrentPage, waitForElementText } from '../helpers/page.js';
import { hostAttach } from '../helpers/poll.js';

const waitForText = (
  app: Parameters<typeof waitForElementText>[0],
  css: string,
  predicate: (text: string) => boolean,
) => waitForElementText(app, 'bridge-repro', css, predicate, 30_000);

spec('keeps bootstrap, calls, and streams healthy across the page bridge', async () => {
  const app = showcaseApp();
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
    await new Promise<void>((resolve) => setTimeout(() => resolve(), 100));
  } catch (error) {
    try {
      const screenshot = await app.page.screenshot({ page: 'bridge-repro' });
      await hostAttach('bridge-repro-failure.png', {
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
      await waitForCurrentPage(app, 'home');
    } catch {
      // Keep a cleanup failure from hiding the original bridge assertion.
    }
  }
});
