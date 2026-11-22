// The View's own words (`@view.*`), generated from i18n/view/. An lxapp's catalogue overrides one by
// naming the same key; a language LingXia does not translate reads English.
import { source, words } from '../generated/words.js';
import { lxappText } from './i18n.js';

export function word(key, language) {
  const own = lxappText(key, language);
  if (own !== undefined) return own;
  const table = words[String(language ?? '').split(/[-_]/)[0].toLowerCase()] ?? words[source];
  return (table[key] ?? words[source][key])({}, null);
}
