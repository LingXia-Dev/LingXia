import { definePage } from '@lingxia/logic';
import type { PullToRefreshPage } from './contract';

const initialData: PullToRefreshPage['data'] = { refreshCount: 0, lastRefreshTime: null, isRefreshing: false };

export default definePage({
  data: initialData,
  setup(page) {
    // The indicator stays until onRefresh settles; "Finish refresh"
    // settles it here, standing in for a fetch.
    let finishRefresh: (() => void) | null = null;
    return {
      onRefresh() {
        const time = new Date().toLocaleTimeString();
        page.update(d => {
          d.isRefreshing = true;
          d.refreshCount += 1;
          d.lastRefreshTime = time;
        });
        return new Promise<void>((resolve) => {
          finishRefresh = () => {
            finishRefresh = null;
            page.update(d => { d.isRefreshing = false; });
            resolve();
          };
        });
      },
      actions: {
        startRefresh() {
          page.refresh();
        },
        stopRefresh() {
          finishRefresh?.();
        },
      },
    };
  },
});
