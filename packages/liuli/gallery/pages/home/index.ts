import { publish, pushEntryWhenNarrow, rememberEntry, rememberedEntry, signIn, takeHandedOff } from '../../shared/pages';

Page({
  data: { page: '' },
  _unwatch: undefined as (() => void) | undefined,
  // `home?page=stat` opens on that page; without one, on the entry the workspace last showed.
  async onLoad(query?: { page?: string }) {
    const page = query?.page || (await rememberedEntry());
    if (page) this.setData({ page });
    this._unwatch = pushEntryWhenNarrow(() => this.data.page);
  },
  onShow() {
    const page = takeHandedOff();
    if (page) this.setData({ page });
  },
  onUnload() {
    this._unwatch?.();
  },
  async open({ page }: { page: string }) {
    await lx.navigateTo({ page });
  },
  // The workspace's choice, kept so the layout that follows a resize or relaunch opens on it.
  async select({ page }: { page: string }) {
    this.setData({ page });
    rememberEntry(page === 'home' ? '' : page);
  },
  async publish() {
    await publish();
  },
  async signIn() {
    await signIn();
  },
});
