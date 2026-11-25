import type { Problem } from '../schema/types';
import { formatWordsWith } from './formatWords';
import generated from './words';

const words = generated as unknown as {
  readonly source: string;
  readonly languages: Readonly<Record<string, Readonly<Record<string, (args: object, f: unknown) => unknown>>>>;
  formatter(tag: string): unknown;
};

/**
 * Jinshu's word `key` in `language`, for Logic, where `jinshuText` cannot run (it words a page's
 * render). An lxapp catalogue does not override these.
 */
export function logicText(language: string, key: `@jinshu.${string}`, args?: Readonly<Record<string, unknown>>): string {
  const primary = String(language).split(/[-_]/)[0].toLowerCase();
  const tag = Object.keys(words.languages).find(t => t.toLowerCase().split('-')[0] === primary) ?? words.source;
  const message = words.languages[tag][key] ?? words.languages[words.source][key];
  if (!message) throw Error(`UI_I18N: no package word '${key}'`);
  const text = message(args ?? {}, words.formatter(tag));
  return Array.isArray(text) ? text.join('') : String(text);
}

/** What `FORMAT` problems lose, as distinct words in `language` for a person ("Highlight", "Audio"); agents keep `message`. */
export function formatProblemWords(problems: readonly Problem[], language: string): string[] {
  return formatWordsWith(problems, (key, args) => logicText(language, key, args));
}
