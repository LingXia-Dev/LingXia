Page({
  data: { packagedMedia: false },
  onLoad(options: { media?: string } = {}) {
    if (options.media === 'packaged') this.setData({ packagedMedia: true });
  },
});
