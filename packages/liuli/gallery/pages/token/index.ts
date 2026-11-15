// Generated from the doc file by scripts/generate.mjs; do not edit.
import { publish, returnToWorkspaceWhenWide, signIn } from '../../shared/pages';

Page({
  _unwatch: undefined as (() => void) | undefined,
  onLoad() {
    this._unwatch = returnToWorkspaceWhenWide("token");
  },
  onUnload() {
    this._unwatch?.();
  },
  async open({ page }: { page: string }) {
    await lx.navigateTo({ page });
  },
  async publish() {
    await publish();
  },
  async signIn() {
    await signIn();
  },
});
