// Generated from the doc file by scripts/generate.mjs; do not edit.
import { definePage } from '@lingxia/types/page';
import { publish, returnToWorkspaceWhenWide, signIn } from '../../shared/pages';

export default definePage({
  setup() {
    let unwatch: (() => void) | undefined;
    return {
      onLoad() {
        unwatch = returnToWorkspaceWhenWide("page-control");
      },
      onUnload() {
        unwatch?.();
      },
      actions: {
        async open({ page }: { page: string }) {
          await lx.navigateTo({ page });
        },
        publish,
        signIn,
      },
    };
  },
});
