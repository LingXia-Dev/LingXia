import { getLanguage } from './runtime-env';

declare global {
  /** The lxapp's messages and their arguments, declared by the generated `.lingxia/types/i18n.generated.d.ts`. */
  interface LxAppMessages {}
}

declare const messageKey: unique symbol;

/** A key built at run time. Cast to it only where a literal key cannot be written. */
export type MessageKey = (keyof LxAppMessages extends never ? string : keyof LxAppMessages & string) & {
  readonly [messageKey]: true;
};

export type MessageCall = keyof LxAppMessages extends never
  ? [key: string, args?: Readonly<Record<string, unknown>>]
  :
      | {
          [K in keyof LxAppMessages & string]: {} extends LxAppMessages[K]
            ? [key: K, args?: LxAppMessages[K]]
            : [key: K, args: LxAppMessages[K]];
        }[keyof LxAppMessages & string]
      | [key: MessageKey, args?: Readonly<Record<string, unknown>>];

type Message = (args: Record<string, unknown>, formatter: unknown) => unknown;

/** What the lxapp build compiles `i18n/` to; the page entry registers it before the page renders. */
export interface CompiledCatalogues {
  source: string;
  languages: Record<string, Record<string, Message>>;
  formatter(tag: string): unknown;
}

let catalogues: CompiledCatalogues | null = null;
const formatters = new Map<string, unknown>();

export function registerCatalogues(compiled: CompiledCatalogues): void {
  catalogues = compiled;
  formatters.clear();
}

/** The page's language narrowed to a catalogue: exact tag, then same language, then the source. */
function catalogueFor(compiled: CompiledCatalogues, language: string): string {
  const tags = Object.keys(compiled.languages);
  const lower = language.toLowerCase();
  const primary = lower.split('-')[0];
  return (
    tags.find((tag) => tag.toLowerCase() === lower) ??
    tags.find((tag) => tag.toLowerCase().split('-')[0] === primary) ??
    compiled.source
  );
}

/**
 * The lxapp's message `key` in the page's language (`getLanguage()`). Reads the language at the call,
 * so re-render on `subscribeLanguage`; React's `useText()` does.
 */
export function t(...[key, args]: MessageCall): string {
  if (!catalogues) throw Error(`UI_I18N: t('${key}') but the lxapp has no i18n/ catalogues`);
  const tag = catalogueFor(catalogues, getLanguage());
  const message = catalogues.languages[tag]?.[key] ?? catalogues.languages[catalogues.source]?.[key];
  if (!message) throw Error(`UI_I18N: no message '${key}'`);
  let formatter = formatters.get(tag);
  if (formatter === undefined) formatters.set(tag, (formatter = catalogues.formatter(tag)));
  const text = message((args ?? {}) as Record<string, unknown>, formatter);
  return Array.isArray(text) ? text.join('') : String(text);
}
