import { labData } from '../../shared/lib/lab-data';

Page({
  data: { ...labData(), lang: '', group: '', dipped: false },
  // Specs pin the page's language with `lang`, and show one group in place with `group`.
  onLoad(options: { lang?: string; group?: string } = {}) {
    if (options.lang) this.setData({ lang: options.lang });
    if (options.group) this.setData({ group: options.group });
  },
  async open({ page }: { page: keyof LxAppPages }) { await lx.navigateTo({ page }); },
  interrupt() {
    this.setData({ dipped: true });
    setTimeout(() => this.setData({ dipped: false }), 100);
  },
});
