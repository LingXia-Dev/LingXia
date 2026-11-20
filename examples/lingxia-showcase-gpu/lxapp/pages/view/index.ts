import { labData } from '../../shared/lib/lab-data';
import { definePage } from '@lingxia/types/page';

export default definePage({
  data: { ...labData(), lang: '', group: '', spec: '', packagedMedia: false, dipped: false },
  setup(thisPage) {
    const actions = {
      async open({ page }: { page: keyof LxAppPages }) { await lx.navigateTo({ page }); },
      interrupt() {
        thisPage.update(d => { d.dipped = true; });
        setTimeout(() => thisPage.update(d => { d.dipped = false; }), 100);
      },
    };
    return {
      // Specs pin the page's language with `lang`, and show one group in place with `group`.
      onLoad(options: { lang?: string; group?: string; spec?: string; media?: string } = {}) {
        const lang = options.lang;
        if (lang) thisPage.update(d => { d.lang = lang; });
        const group = options.group;
        if (group) thisPage.update(d => { d.group = group; });
        const spec = options.spec;
        if (spec) thisPage.update(d => {
          d.spec = spec;
          d.packagedMedia = options.media === 'packaged';
        });
      },
      actions,
    };
  },
});
