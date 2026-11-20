import { publish, pushEntryWhenNarrow, rememberEntry, rememberedEntry, signIn, takeHandedOff } from '../../shared/pages';
import { definePage } from '@lingxia/types/page';

export default definePage({
  data: { page: '' },
  setup(thisPage) {
    let unwatch = undefined as (() => void) | undefined;
    const actions = {
      async open({ page }: { page: string }) {
        await lx.navigateTo({ page });
      },
      // The workspace's choice, kept so the layout that follows a resize or relaunch opens on it.
      async select({ page }: { page: string }) {
        thisPage.update(d => { d.page = page; });
        rememberEntry(page === 'home' ? '' : page);
      },
      async publish() {
        await publish();
      },
      async signIn() {
        await signIn();
      },
    };
    return {
      // `home?page=stat` opens on that page; without one, on the entry the workspace last showed.
      async onLoad(query?: { page?: string }) {
        const page = query?.page || (await rememberedEntry());
        if (page) thisPage.update(d => { d.page = page; });
        unwatch = pushEntryWhenNarrow(() => thisPage.data.page);
      },
      onShow() {
        const page = takeHandedOff();
        if (page) thisPage.update(d => { d.page = page; });
      },
      onUnload() {
        unwatch?.();
      },
      actions,
    };
  },
});
