// `t(key, args)`: the lxapp's message in the page's language. The lxapp build compiles each
// `i18n/<tag>.json` to one function per message and the page compiler registers them here.
import { render } from './state.js';
import { envOf } from './hooks.js';

let catalogues = null;

/** `{ source, languages: { [tag]: { [key]: (args, f) => text } }, formatter(tag) }`, from the page compiler. */
export function registerCatalogues(compiled) { catalogues = compiled; formatters.clear(); }

/** The page's language narrowed to a catalogue: exact tag, then same language, then the source. */
function catalogueFor(language) {
  const tags = Object.keys(catalogues.languages);
  const lower = String(language ?? '').toLowerCase();
  const exact = tags.find(tag => tag.toLowerCase() === lower);
  const primary = lower.split('-')[0];
  const tag = exact ?? tags.find(tag => tag.toLowerCase().split('-')[0] === primary) ?? catalogues.source;
  return { tag, messages: catalogues.languages[tag] };
}

export function t(key, args) {
  if (!render.active) throw Error('UI_SCOPE: t outside render');
  if (!catalogues) throw Error(`UI_I18N: t('${key}') but the lxapp has no i18n/ catalogues`);
  // The memo stamp covers the whole environment, so a language switch re-renders every caller.
  const env = envOf(render.view);
  const { tag, messages } = catalogueFor(env?.contentLanguage ?? env?.displayLanguage);
  const message = messages[key] ?? catalogues.languages[catalogues.source][key];
  if (!message) throw Error(`UI_I18N: no message '${key}'`);
  return message(args ?? {}, formatter(tag));
}

const formatters = new Map();
function formatter(tag) {
  let f = formatters.get(tag);
  if (!f) formatters.set(tag, f = catalogues.formatter(tag));
  return f;
}
