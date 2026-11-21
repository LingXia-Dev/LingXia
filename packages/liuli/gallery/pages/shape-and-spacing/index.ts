// Generated from the doc file by scripts/generate.mjs; do not edit.
import { definePage, lx } from '@lingxia/logic';
import { publish, returnToWorkspaceWhenWide, signIn } from '../../shared/pages';

export default definePage({
  setup(thisPage) {
    let unwatch: (() => void) | undefined;
    return {
      onLoad() {
        unwatch = returnToWorkspaceWhenWide("shape-and-spacing", thisPage.env);
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
