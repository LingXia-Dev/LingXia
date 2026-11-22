import { labData } from '../../shared/lib/lab-data';
import { definePage } from '@lingxia/logic';

export default definePage({
  data: { ...labData(), group: '', spec: '', packagedMedia: false, dipped: false },
  setup(thisPage) {
    const actions = {
      interrupt() {
        thisPage.update(d => { d.dipped = true; });
        setTimeout(() => thisPage.update(d => { d.dipped = false; }), 100);
      },
    };
    return {
      // Specs show one group in place with `group`.
      onLoad(options: { group?: string; spec?: string; media?: string } = {}) {
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
