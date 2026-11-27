import { expect, test } from '@lingxia/test';
import { bindFixture } from '@lingxia/test/showcase/helpers/poll.js';
import { SHOWCASE_APP_ID } from '@lingxia/test/showcase/helpers/app.js';

test('a caught failure toasts in its code\'s words, danger; a cancel shows nothing', {
  id: 'UI-ERROR-001',
  covers: ['lx.ui.toast', 'lx.ui.alert'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'UI-ERROR-001');
  // One toast shows at a time and a newer one replaces it, so each step waits for its record.
  const toast = (failure: { code: string; plain?: boolean }, options?: { fallback?: string; tone?: 'default' }) =>
    app.logic.eval(({ lx }, failure, options) => {
      const error = failure.plain ? { code: failure.code } : Object.assign(new Error(`english ${failure.code}`), { code: failure.code });
      try { throw error; } catch (e) { lx.ui.toast(e, options ?? undefined); }
    }, failure, options ?? null);
  const recorded = async (count: number) => {
    await expect.poll(async () => (await app.dialogs.toasts()).length).toBe(count);
    return (await app.dialogs.toasts())[count - 1];
  };
  await toast({ code: 'E_ABORT' });
  await toast({ code: 'E_NETWORK' });
  const network = await recorded(1);
  await toast({ code: 'E_INTERNAL' }, { fallback: "Couldn't save the note", tone: 'default' });
  const saved = await recorded(2);
  await toast({ code: 'E_TIMEOUT', plain: true });
  const timeout = await recorded(3);
  // The cancel left no record; the words come from the code, never the English `message`.
  expect([network.tone, saved.tone, timeout.tone]).toEqual(['danger', 'default', 'danger']);
  expect(saved.message).toBe("Couldn't save the note");
  for (const { message } of [network, timeout]) expect(message).not.toMatch(/^$|english/);
  expect(network.message).not.toBe(timeout.message);
  await app.logic.eval(async ({ lx }) => {
    await lx.ui.alert({ error: Object.assign(new Error('stop'), { code: 'E_ABORT' }) });
  });
  expect(await app.dialogs.modals()).toEqual([]);

  for (let i = 0; i < 3; i++) await app.dialogs.answerNextModal({ confirm: true });
  await app.logic.eval(async ({ lx }) => {
    const quota = Object.assign(new Error('english'), { code: 'E_QUOTA' });
    await lx.ui.alert({ error: quota, fallback: 'unused' });
    await lx.ui.alert({ error: new Error('english'), fallback: "Couldn't save the note" });
    await lx.ui.alert({ error: quota, title: 'Storage is full', message: 'Delete a note to make room.' });
  });
  const [limit, ...rest] = (await app.dialogs.modals()).map(({ title }) => title);
  expect(limit).not.toMatch(/^$|english|unused/);
  expect(rest).toEqual(["Couldn't save the note", 'Storage is full']);
});
