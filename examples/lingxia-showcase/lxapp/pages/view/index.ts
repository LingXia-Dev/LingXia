import { definePage } from '@lingxia/logic';
import { heldSnap, holdPage, holdPanel, type HeldChoice } from '../../shared/lib/held-snap';

export default definePage({
  data: { group: '', spec: '', packagedMedia: false, dipped: false, held: heldSnap() },
  setup(thisPage) {
    const actions = {
      holdPage: (choice: HeldChoice) => holdPage(thisPage, choice),
      holdPanel: (choice: HeldChoice) => holdPanel(thisPage, choice),
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
