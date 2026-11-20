// Generated from the doc file by scripts/generate.mjs; do not edit.
import { publish, returnToWorkspaceWhenWide, signIn } from '../../shared/pages';
import { definePage } from '@lingxia/types/page';

export default definePage({
  setup() {
    let unwatch = undefined as (() => void) | undefined;
    const actions = {
      async open({ page }: { page: string }) {
        await lx.navigateTo({ page });
      },
      async publish() {
        await publish();
      },
      async signIn() {
        await signIn();
      },
    };
    return {
      onLoad() {
        unwatch = returnToWorkspaceWhenWide("stat");
      },
      onUnload() {
        unwatch?.();
      },
      actions,
    };
  },
});
