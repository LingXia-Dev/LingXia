// For specs only: records what navigation tells this page (its first visibility, each change of
// `page.visible`, the params of each `onShow` and `page.params` then), for the reuse and
// visibility checks. A spec that sets `globalThis.navOrder` also gets each onShow there.
import { definePage } from '@lingxia/logic';

export default definePage({
  data: {
    firstVisible: null as boolean | null,
    visibleLog: [] as boolean[],
    shows: [] as (Record<string, unknown> | null)[],
    current: [] as Record<string, unknown>[],
  },
  setup(page) {
    const firstVisible = page.visible.get();
    const visibleLog: boolean[] = [];
    let loaded = false;
    page.visible.watch((visible) => {
      visibleLog.push(visible);
      if (loaded) page.set({ visibleLog: [...visibleLog] });
    });
    return {
      onLoad() {
        loaded = true;
        page.set({ firstVisible, visibleLog: [...visibleLog] });
      },
      onShow(options) {
        const order = (globalThis as { navOrder?: string[] }).navOrder;
        order?.push(`show:${JSON.stringify(options.params ?? null)}`);
        page.update((d) => {
          d.shows.push((options.params as Record<string, unknown> | undefined) ?? null);
          d.current.push({ ...(page.params as Record<string, unknown>) });
        });
      },
    };
  },
});
