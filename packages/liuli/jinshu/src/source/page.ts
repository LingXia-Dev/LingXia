import type { JinshuSource, JinshuSourceState, MinimalChannelEndpoint } from './types';

export interface MinimalPageContext {
  update(patch: Record<string, unknown> | ((draft: any) => void)): void;
  onUnload?(fn: () => void): void;
}

/**
 * Shows a shared source on a page: its compact flags go through `page.update` and `syncDoc` serves
 * its channel. Unloading the page stops the flags only; the source and its draft live on for other
 * pages and agents.
 */
export function bindJinshuPage(
  page: MinimalPageContext,
  source: JinshuSource,
): { readonly syncDoc: (_params: unknown, channel: MinimalChannelEndpoint) => void } {
  const pushFlags = (s: JinshuSourceState) => {
    page.update({
      dirty: s.dirty,
      saving: s.saving,
      saveStatus: s.saveStatus,
      docProblems: s.problems.length,
      pendingSuggestions: s.suggestions.length,
    });
  };
  pushFlags(source.getState());
  const unsub = source.subscribe(pushFlags);
  page.onUnload?.(unsub);
  return {
    syncDoc(_params, channel) {
      source.serve(channel);
    },
  };
}
