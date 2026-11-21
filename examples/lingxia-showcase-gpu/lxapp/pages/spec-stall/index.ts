// For specs only: under `lingxia dev` this page's first state is held (index.json), so its View
// waits into the "taking longer to load" panel. Outside a dev session it opens normally.
import { definePage } from '@lingxia/types/page';
export default definePage({
  data: { ready: true },
  setup() {
    return {
    };
  },
});
