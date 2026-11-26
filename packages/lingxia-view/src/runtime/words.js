// The View's own words (`@view.*`), generated from i18n/view/. An lxapp's catalogue overrides one by
// naming the same key; a language LingXia does not translate reads English.
import { source, words } from '../generated/words.js';
import { lxappText } from './i18n.js';
import { render, page } from './state.js';
import { envOf } from './hooks.js';
import { noteEnv } from './memo.js';

export function word(key, language) {
  const own = lxappText(key, language);
  if (own !== undefined) return own;
  const table = words[String(language ?? '').split(/[-_]/)[0].toLowerCase()] ?? words[source];
  return (table[key] ?? words[source][key])({}, null);
}

// Codes a person can act on get their own word; the rest read as a plain failure.
const ERROR_WORDS = {
  E_NETWORK: 'network', VIDEO_NETWORK: 'network', WEBVIEW_NETWORK: 'network',
  E_TIMEOUT: 'timeout',
  E_PERMISSION_DENIED: 'permission', VIDEO_NOT_GRANTED: 'permission', WEBVIEW_NOT_GRANTED: 'permission',
  E_NOT_SUPPORTED: 'unsupported', E_CAPABILITY_MISSING: 'unsupported', VIDEO_UNSUPPORTED: 'unsupported',
  VIDEO_NO_PLAYER: 'unsupported', VIDEO_DECODERS: 'unsupported', WEBVIEW_UNSUPPORTED: 'unsupported',
  E_NOT_FOUND: 'notFound', VIDEO_NOT_FOUND: 'notFound',
  E_QUOTA: 'limit',
};

/** A failure as text for the person, by its `code`; `fallback` when no code-specific word applies; `null` for `E_ABORT`, a cancel. */
export function errorText(error, fallback) {
  const code = typeof error?.code === 'string' ? error.code : undefined;
  if (code === 'E_ABORT') return null;
  const view = render.active ? render.view : page.committed?.view;
  if (!view) throw Error('UI_SCOPE: errorText before the page\'s first frame; call it in render or an event handler');
  if (render.active) noteEnv('language');
  const specific = ERROR_WORDS[code];
  if (!specific && fallback !== undefined) return fallback;
  return word(`@view.error.${specific ?? 'failed'}`, envOf(view)?.language);
}
