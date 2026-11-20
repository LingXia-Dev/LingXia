import { definePage } from '@lingxia/types/page';
export default definePage({
  data: {
    refreshCount: 0,
    lastRefreshTime: null as string | null,
    isRefreshing: false,
  },
  setup(page) {
    const actions = {
      startRefresh() {
        console.log("startPullDownRefresh called");
        lx.startPullDownRefresh();
      },
      stopRefresh() {
        console.log("stopPullDownRefresh called");
        page.update(d => { d.isRefreshing = false; });
        lx.stopPullDownRefresh();
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
        console.log("onPullDownRefresh triggered");
        const now = new Date();
        const timeString = now.toLocaleTimeString();

        page.update(d => {
          d.isRefreshing = true;
          d.refreshCount = page.data.refreshCount + 1;
          d.lastRefreshTime = timeString;
        });
      },
      actions,
    };
  },
});
