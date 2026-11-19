// For specs only: the page shows, then an action turns `broken` on and every later render
// throws, so the host keeps the last frame under its "couldn't update" banner.
Page({
  data: { broken: false },
  breakView() {
    this.setData({ broken: true });
  },
});
