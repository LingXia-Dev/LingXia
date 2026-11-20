// Logic every gallery page shares. A wide window shows the gallery as one workspace on the home page
// and a narrow one as pushed pages, so the open entry follows the layout: the workspace pushes its
// entry's page when the window is narrow, and a pushed page returns to the workspace, selected there,
// when it is wide. The build reads a page's actions from what its own `setup` returns, so pages
// call these from actions written there.

const wait = (ms: number) => new Promise<void>(resolve => setTimeout(resolve, ms));

/** Stands in for a real Logic action, so Button's AsyncAction example shows its loading state. */
export async function publish() {
  await wait(1200);
}

/** Stands in for a real sign-in request, so the Sign in pattern shows its busy state. */
export async function signIn() {
  await wait(1200);
}

/** The entry a pushed page hands to the workspace; home picks it up when it shows again. */
let handedOff = '';

// The workspace's entry, kept across a relaunch of the lxapp's Logic (the Runner relaunches it when its
// device changes), so the layout that comes back opens the same page.
const entryKey = 'gallery.entry';

/** Remembers the workspace's entry, or forgets it with ''. */
export function rememberEntry(page: string): void {
  const storage = lx.getStorage();
  void (page ? storage.set(entryKey, page) : storage.delete(entryKey));
}

/** The remembered workspace entry, or ''. */
export async function rememberedEntry(): Promise<string> {
  const page = await lx.getStorage().get<string>(entryKey);
  return typeof page === 'string' ? page : '';
}

type SizeClass = 'compact' | 'regular';

/** Calls `onClass(sizeClass, crossed)` with the class the page opened in, then on every change of it. */
function watchClass(onClass: (sizeClass: SizeClass, crossed: boolean) => void): () => void {
  let last: SizeClass | null = null;
  return lx.surface.watchContext(context => {
    if (context.sizeClass === last) return;
    const crossed = last !== null;
    last = context.sizeClass;
    // Off the subscribe call, which runs inside onLoad, so navigation never starts mid-load.
    setTimeout(() => onClass(context.sizeClass, crossed), 0);
  });
}

/**
 * A pushed page: in a wide window, hand `page` to the workspace and go back to it, whether the window
 * widened or the page came back wide. Returns the unsubscribe.
 */
export function returnToWorkspaceWhenWide(page: string): () => void {
  return watchClass(sizeClass => {
    const depth = getCurrentPages().length;
    if (sizeClass !== 'regular' || depth < 2) return;
    handedOff = page;
    rememberEntry(page);
    void lx.nav.back(depth - 1);
  });
}

/**
 * Home: in a narrow window with an entry open, push that entry's page, whether the window narrowed or
 * home came back narrow with a remembered entry. Returns the unsubscribe.
 */
export function pushEntryWhenNarrow(entry: () => string): () => void {
  return watchClass(sizeClass => {
    const page = entry();
    if (sizeClass !== 'compact' || !page || page === 'home' || getCurrentPages().length > 1) return;
    rememberEntry('');
    void lx.nav.go({ page });
  });
}

/** The entry a pushed page handed over, once; empty when none. */
export function takeHandedOff(): string {
  const page = handedOff;
  handedOff = '';
  return page;
}
