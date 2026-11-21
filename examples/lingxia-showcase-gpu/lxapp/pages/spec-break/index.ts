// For specs only: the page shows, then an action turns `broken` on and every later render
// throws, so the host keeps the last frame under its "couldn't update" banner.
import { definePage } from '@lingxia/types/page';
export default definePage({
  data: { broken: false },
  setup(page) {
    const actions = {
      breakView() {
        page.update(d => { d.broken = true; });
      },
    };
    return {
      actions,
    };
  },
});
