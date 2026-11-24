// `t(key, args)`: the lxapp's message in the page's language. The lxapp build compiles each
// `i18n/<tag>.json` to one function per message and the page compiler registers them here.
import { render } from './state.js';
import { envOf } from './hooks.js';

let catalogues = null;

/** `{ source, languages: { [tag]: { [key]: (args, f) => text } }, formatter(tag) }`, from the page compiler. */
export function registerCatalogues(compiled) { catalogues = compiled; formatters.clear(); }

/** Registers one per-language catalogue module (`lx:view/<id>/<tag>`), evaluated by the host. */
export function registerLanguageCatalogue(tag, messages, formatterFn, source) {
  if (!catalogues) catalogues = { source: source ?? tag, languages: {}, formatter: formatterFn };
  else {
    if (source) catalogues.source = source;
    if (formatterFn) catalogues.formatter = formatterFn;
  }
  catalogues.languages[tag] = messages;
  formatters.delete(tag);
}

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
  const { tag, messages } = catalogueFor(env?.language);
  const message = messages?.[key] ?? catalogues.languages[catalogues.source]?.[key];
  if (!message) throw Error(`UI_I18N: no message '${key}'`);
  return message(args ?? {}, formatter(tag));
}

export function packageText(words, key, args) {
  if (!render.active) throw Error('UI_SCOPE: packageText outside render');
  const language = envOf(render.view)?.language;
  const own = lxappText(key, language, args);
  if (own !== undefined) return own;
  const primary = String(language ?? '').split(/[-_]/)[0].toLowerCase();
  const tag = Object.keys(words.languages).find(tag => tag.toLowerCase().split('-')[0] === primary) ?? words.source;
  const message = words.languages[tag][key] ?? words.languages[words.source][key];
  if (!message) throw Error(`UI_I18N: no package word '${key}'`);
  return join(message(args ?? {}, words.formatter(tag)));
}

const join = text => Array.isArray(text) ? text.join('') : String(text);

/** The lxapp's own `@view.*` word in the page's catalogue. Only that catalogue's: an override in
 *  the source language must not replace the View's word in another. */
export function lxappText(key, language, args) {
  if (!catalogues) return undefined;
  const { tag, messages } = catalogueFor(language);
  const message = messages[key];
  return message ? join(message(args ?? {}, formatter(tag))) : undefined;
}

const formatters = new Map();
function formatter(tag) {
  let f = formatters.get(tag);
  if (!f) formatters.set(tag, f = catalogues.formatter(tag));
  return f;
}
