import { definePage, lx } from '@lingxia/logic';
export default definePage({
  data: { lang: '', dipped: false },
  setup(thisPage) {
    const actions = {
      async open({ page }: { page: keyof LxAppPages }) { await lx.nav.go({ page }).opened; },
      // Reverses the fade 100 ms in, sooner than two taps can land.
      interrupt() {
        thisPage.update(d => { d.dipped = true; });
        setTimeout(() => thisPage.update(d => { d.dipped = false; }), 100);
      },
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
