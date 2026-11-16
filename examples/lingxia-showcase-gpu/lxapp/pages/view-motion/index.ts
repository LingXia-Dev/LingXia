Page({
  data: { lang: '', dipped: false },
  // Specs pin the page's language with `lang`, whatever the device is set to.
  onLoad(options: { lang?: string } = {}) {
    if (options.lang) this.setData({ lang: options.lang });
  },
  async open({ page }: { page: keyof LxAppPages }) { await lx.navigateTo({ page }); },
  // Reverses the fade 100 ms in, sooner than two taps can land.
  interrupt() {
    this.setData({ dipped: true });
    setTimeout(() => this.setData({ dipped: false }), 100);
  },
});
