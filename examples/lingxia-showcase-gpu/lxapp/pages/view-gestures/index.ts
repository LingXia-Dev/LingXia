import { definePage } from '@lingxia/types/page';
export default definePage({
  data: { lang: '' },
  setup(thisPage) {
    const actions = {
      async open({ page }: { page: keyof LxAppPages }) { await lx.navigateTo({ page }); },
    };
    return {
      // Specs pin the page's language with `lang`, whatever the device is set to.
      onLoad(options: { lang?: string } = {}) {
        const lang = options.lang;
        if (lang) thisPage.update(d => { d.lang = lang; });
      },
      actions,
    };
  },
});
