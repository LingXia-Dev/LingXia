import type { AbortSignalLike } from './signal';
import type { ChannelParams, JinshuSource, JinshuSourceState, MinimalChannelEndpoint } from './types';

/** The part of a Logic page (`definePage`'s `setup(page)`) the binding uses. */
export interface MinimalPageContext {
  update(recipe: (draft: any) => void): void;
  /** Aborted as the page unloads. */
  readonly signal?: AbortSignalLike;
}

/** The flags `bindJinshuPage` keeps in the page's data, for the page's own chrome. */
export interface JinshuPageFlags {
  readonly dirty: boolean;
  readonly saving: boolean;
  readonly saveStatus: JinshuSourceState['saveStatus'];
  readonly docProblems: number;
  readonly pendingSuggestions: number;
}

/**
 * Shows a shared source on a page: its compact flags go into the page's data and `syncDoc` serves
 * its channel. Unloading the page stops the flags only; the source and its draft live on for other
 * pages and agents.
 */
export function bindJinshuPage(
  page: MinimalPageContext,
  source: JinshuSource,
): { readonly syncDoc: (params: unknown, channel: MinimalChannelEndpoint) => void } {
  const pushFlags = (s: JinshuSourceState) => {
    const flags: JinshuPageFlags = {
      dirty: s.dirty,
      saving: s.saving,
      saveStatus: s.saveStatus,
      docProblems: s.problems.length,
      pendingSuggestions: s.suggestions.length,
    };
    page.update(d => void Object.assign(d, flags));
  };
  pushFlags(source.getState());
  const unsub = source.subscribe(pushFlags);
  page.signal?.addEventListener('abort', unsub, { once: true });
  return {
    syncDoc(params, channel) {
      source.serve(channel, params as ChannelParams | undefined);
    },
  };
}
