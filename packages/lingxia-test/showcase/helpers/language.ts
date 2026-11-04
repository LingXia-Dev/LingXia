import type { Fixture } from '@lingxia/test';
import { SHOWCASE_APP_ID } from './app.js';
import { eventually } from './poll.js';

/**
 * The host chrome these specs press is labelled in English. Pin the app's display language to
 * en-US for the spec and put the user's own back afterwards, also when the spec fails or is
 * interrupted: the restore is deferred before the switch.
 */
export async function pinEnglish(t: Fixture): Promise<void> {
  const app = t.automation.lxapp(SHOWCASE_APP_ID);
  const preference = () =>
    app.logic.eval(({ lx }) => lx.host.control?.displayLanguage?.getPreference?.() ?? null);
  const previous = await preference();
  if (previous === null || previous === 'en-US') return;
  t.defer(async () => {
    await app.logic.eval(({ lx }, language) => lx.host.control?.displayLanguage?.setPreference(language), previous);
  });
  await app.logic.eval(({ lx }) => lx.host.control?.displayLanguage?.setPreference('en-US'));
  await eventually(preference, (value) => value === 'en-US', {
    describe: 'display language pinned to en-US',
    timeoutMs: 15_000,
  });
}
