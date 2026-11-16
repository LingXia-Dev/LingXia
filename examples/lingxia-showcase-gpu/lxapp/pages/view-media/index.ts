import { labData } from '../../shared/lib/lab-data';

Page({
  data: { ...labData(), lang: '' },
  // Specs pin the page's language with `lang`, whatever the device is set to.
  onLoad(options: { lang?: string } = {}) {
    if (options.lang) this.setData({ lang: options.lang });
  },
});
