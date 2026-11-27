import { expect, test } from '@lingxia/test';
import { bindFixture } from '@lingxia/test/showcase/helpers/poll.js';
import { SHOWCASE_APP_ID } from '@lingxia/test/showcase/helpers/app.js';

test('a caught failure toasts as its errorText words, danger; a cancel shows nothing', {
  id: 'UI-ERROR-001',
  covers: ['lx.ui.toast', 'lx.ui.alert', 'lx.app.errorText'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'UI-ERROR-001');
  const words = await app.logic.eval(async ({ lx }) => {
    const fail = (code: string) => Object.assign(new Error(`english ${code}`), { code });
    const show = (error: unknown, options?: { fallback?: string; tone?: 'default' }) => {
      try { throw error; } catch (e) { lx.ui.toast(e, options); }
    };
    show(fail('E_ABORT'));
    show(fail('E_NETWORK'));
    show(fail('E_INTERNAL'), { fallback: "Couldn't save the note", tone: 'default' });
    lx.ui.toast({ code: 'E_TIMEOUT' });
    await lx.ui.alert(fail('E_ABORT'));
    return {
      network: lx.app.errorText(fail('E_NETWORK')),
      timeout: lx.app.errorText({ code: 'E_TIMEOUT' }),
    };
  });
  await expect.poll(async () => (await app.dialogs.toasts()).map(({ message, tone }) => [message, tone]))
    .toEqual([
      [words.network, 'danger'],
      ["Couldn't save the note", 'default'],
      [words.timeout, 'danger'],
    ]);
  expect(await app.dialogs.modals()).toEqual([]);

  await app.dialogs.answerNextModal({ confirm: true });
  await app.dialogs.answerNextModal({ confirm: true });
  const quota = await app.logic.eval(async ({ lx }) => {
    const quota = Object.assign(new Error('english'), { code: 'E_QUOTA' });
    await lx.ui.alert(quota, { fallback: 'unused' });
    await lx.ui.alert(new Error('english'), { fallback: "Couldn't save the note" });
    return lx.app.errorText(quota);
  });
  expect((await app.dialogs.modals()).map(({ title }) => title)).toEqual([quota, "Couldn't save the note"]);
});
