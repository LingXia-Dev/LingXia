import { expect, spec } from '@lingxia/test';
import type { PageContract } from '@lingxia/types/page';
import { SHOWCASE_APP_ID } from '../helpers/app.js';

for (const mode of ['auth', 'mqtt', 'functions'] as const) {
  spec(`render cloud ${mode} state and controls without a cloud account`, {
    id: `CLOUD-VIEW-${mode.toUpperCase()}-001`,
    covers: ['lx.navigateTo'],
    app: SHOWCASE_APP_ID,
  }, async (t) => {
    t.defer(async () => { await t.app.nav.relaunch({ page: 'home' }); });
    await t.app.nav.relaunch({ page: 'cloud', query: { type: mode } });
    const page = await t.app.page<PageContract<Record<string, unknown>>>({ name: 'cloud' });
    await expect(page.view.testId('cloud-page')).toHaveAttribute('data-mode', mode);
    const key = mode === 'auth' ? 'status' : mode === 'mqtt' ? 'mqttStatus' : 'functionsStatus';
    await t.waitFor(async () => {
      const status = (await page.data())[key];
      const rendered = await page.view.testId(`cloud-${mode}-status`).textContent();
      return typeof status === 'string' && status !== '' && status !== 'Idle'
        && rendered?.trim() === status;
    }, { until: matches => matches });
    for (const other of ['auth', 'mqtt', 'functions']) {
      if (other !== mode) await expect(page.view.testId(`cloud-${other}-status`)).toHaveCount(0);
    }
    if (mode === 'auth') {
      await expect(page.view.testId('cloud-add-identity')).toHaveText('Add Identity');
      await expect(page.view.testId('cloud-logout')).toHaveText('Logout Current Tenant');
    } else if (mode === 'mqtt') {
      await expect(page.view.testId('cloud-subscribe')).toHaveText('Subscribe');
      await expect(page.view.testId('cloud-unsubscribe')).toHaveText('Unsubscribe');
    } else {
      for (const name of ['echo', 'whoami', 'fail']) {
        await expect(page.view.css(`[data-testid="cloud-function"][data-function="${name}"]`)).toHaveText(`Call ${name}`);
      }
    }
  });
}
