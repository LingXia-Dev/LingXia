import { definePage } from '@lingxia/logic';
export default definePage({
  data: { lang: '' },
  setup(thisPage) {
    return {
      // Specs pin the page's language with `lang`, whatever the device is set to.
      onLoad(options: { lang?: string } = {}) {
        const lang = options.lang;
        if (lang) thisPage.update(d => { d.lang = lang; });
      },
    };
  },
});
