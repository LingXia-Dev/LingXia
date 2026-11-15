Page({
  data: {},
  // Specs pin the page's language with `lang`, whatever the device is set to.
  onLoad(options: { lang?: string } = {}) {
    if (options.lang) this.setData({ lang: options.lang });
  },
});
