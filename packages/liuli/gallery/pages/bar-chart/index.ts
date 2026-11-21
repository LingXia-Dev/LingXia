// Generated from the doc file by scripts/generate.mjs; do not edit.
import { definePage } from '@lingxia/types/page';
import { publish, returnToWorkspaceWhenWide, signIn } from '../../shared/pages';

export default definePage({
  setup(thisPage) {
    let unwatch: (() => void) | undefined;
    return {
      onLoad() {
        unwatch = returnToWorkspaceWhenWide("bar-chart", thisPage.env);
      },
      onUnload() {
        unwatch?.();
      },
      actions: {
        async open({ page }: { page: string }) {
          await lx.nav.go({ page }).opened;
        },
        publish,
        signIn,
      },
    };
  },
});
