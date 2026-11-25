import { createJinshuSource } from './source';
import type { JinshuSource, JinshuSourceOptions } from './types';

export interface JinshuSources {
  /**
   * The one source for `key`, made on first use: a stored draft is restored, else the document is
   * loaded. Pages and agent tools open the same key and share its draft; `null` for an unknown key.
   */
  open(key: string): Promise<JinshuSource | null>;
  /** The source for `key` if something already opened it, without loading anything. */
  peek(key: string): JinshuSource | undefined;
}

/** Keeps one source per document key for as long as Logic runs; no page owns one. */
export function createJinshuSources(
  optionsFor: (key: string) => Omit<JinshuSourceOptions, 'key'> | null,
): JinshuSources {
  const sources = new Map<string, JinshuSource>();
  const ready = new Map<string, Promise<JinshuSource | null>>();
  return {
    open(key) {
      const known = ready.get(key);
      if (known) return known;
      const options = optionsFor(key);
      if (!options) return Promise.resolve(null);
      const source = createJinshuSource({ ...options, key });
      sources.set(key, source);
      const opened = source
        .restoreDraft()
        .then(restored => (restored || !options.load ? source : source.load().then(() => source)));
      ready.set(key, opened);
      return opened;
    },
    peek: key => sources.get(key),
  };
}
