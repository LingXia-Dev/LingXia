import { expect, test } from '@lingxia/test';

test('button: an async action reads busy while it runs, then not', {
  id: 'LOCATOR-BUSY-001', start: { page: 'button' }, timeout: 60_000,
}, async (t) => {
  const publish = t.app.view.testId('publish');
  await expect(publish).not.toBeBusy();
  await publish.click();
  await expect(publish).toBeBusy();
  await expect(publish).not.toBeBusy({ timeout: 5_000 });
});
