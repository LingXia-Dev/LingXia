import type { Fixture } from '@lingxia/test';

/** Shows the app in `language` for the spec, then puts the user's preference back, also on failure. */
export async function inLanguage(t: Fixture, language: string): Promise<void> {
  const previous = await t.app.logic.eval(({ lx }) => lx.host.languagePreference.get());
  if (previous === language) return;
  t.defer(async () => { await t.app.logic.eval(async ({ lx }, back) => { await lx.host.languagePreference.set(back); }, previous); });
  await t.app.logic.eval(async ({ lx }, next) => { await lx.host.languagePreference.set(next); }, language);
}
