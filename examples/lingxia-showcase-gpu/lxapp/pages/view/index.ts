import { labData } from '../../shared/lib/lab-data';

Page({
  data: { ...labData(), lang: '', group: '', spec: '', packagedMedia: false, dipped: false },
  // Specs pin the page's language with `lang`, and show one group in place with `group`.
  onLoad(options: { lang?: string; group?: string; spec?: string; media?: string } = {}) {
    if (options.lang) this.setData({ lang: options.lang });
    if (options.group) this.setData({ group: options.group });
    if (options.spec) this.setData({ spec: options.spec, packagedMedia: options.media === 'packaged' });
  },
  async open({ page }: { page: keyof LxAppPages }) { await lx.navigateTo({ page }); },
  interrupt() {
    this.setData({ dipped: true });
    setTimeout(() => this.setData({ dipped: false }), 100);
  },
});
