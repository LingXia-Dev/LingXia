import { definePage } from '@lingxia/logic';
export default definePage({
  data: {
    refreshCount: 0,
    lastRefreshTime: null as string | null,
    isRefreshing: false,
  },
  setup(page) {
    // The indicator stays until onPullDownRefresh settles; the stop button
    // settles it here, standing in for a fetch.
    let finishRefresh: (() => void) | null = null;
    const actions = {
      startRefresh() {
        page.refresh();
      },
      stopRefresh() {
        finishRefresh?.();
      },
    };
    return {
      onLoad(options) {
        console.log("PullDownRefresh page onLoad");
      },
      onShow() {
        console.log("PullDownRefresh page onShow");
      },
      onHide() {
        console.log("PullDownRefresh page onHide");
      },
      // Page lifecycle: Called when user pulls down to refresh
      onPullDownRefresh() {
        const timeString = new Date().toLocaleTimeString();
        page.update(d => {
          d.isRefreshing = true;
          d.refreshCount = page.data.refreshCount + 1;
          d.lastRefreshTime = timeString;
        });
        return new Promise<void>((resolve) => {
          finishRefresh = () => {
            finishRefresh = null;
            page.update(d => { d.isRefreshing = false; });
            resolve();
          };
        });
      },
      actions,
    };
  },
});
