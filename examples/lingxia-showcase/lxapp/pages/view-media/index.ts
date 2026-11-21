import { labData } from '../../shared/lib/lab-data';
import { definePage } from '@lingxia/logic';

export default definePage({
  data: { ...labData(), lang: '' },
  setup(page) {
    return {
      // Specs pin the page's language with `lang`, whatever the device is set to.
      onLoad(options: { lang?: string } = {}) {
        const lang = options.lang;
        if (lang) page.update(d => { d.lang = lang; });
      },
    };
  },
});
