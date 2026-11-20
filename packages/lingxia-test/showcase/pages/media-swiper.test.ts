import { expect, test, type Fixture, type TestView } from '@lingxia/test';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { currentPageOrNull, waitForCurrentPage, waitForCurrentPageVisible } from '../helpers/page.js';
import { bindFixture, eventually } from '../helpers/poll.js';

const SWIPER = '#lx-media-swiper-demo';
// 64x40 solid red, green and blue PNGs: three pages that need no network and no picker.
const PAGES = [
  ['red', 'iVBORw0KGgoAAAANSUhEUgAAAEAAAAAoCAIAAADBrGu+AAAAPUlEQVR42u3PQQkAAAgEsIt2/QOYxww+hcEKLNO+FgEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQGBiwX4qFzEMNmn2gAAAABJRU5ErkJggg=='],
  ['green', 'iVBORw0KGgoAAAANSUhEUgAAAEAAAAAoCAIAAADBrGu+AAAAPUlEQVR42u3PQQkAAAgEsItmDoOZ1ww+hcEKLDX9WgQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQELhYYgeCmLlvnXAAAAABJRU5ErkJggg=='],
  ['blue', 'iVBORw0KGgoAAAANSUhEUgAAAEAAAAAoCAIAAADBrGu+AAAAPUlEQVR42u3PQQkAAAgEsItmFFObxww+hcEKLNXzWgQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQELhZayZzTnRPLvQAAAABJRU5ErkJggg=='],
] as const;

interface Reported {
  type: string;
  index: number | null;
  previousIndex: number | null;
  source: string | null;
  item: string | null;
}

/** Every event the swiper element has dispatched since the recorder was installed. */
function reported(view: TestView): Promise<Reported[]> {
  return view.eval(({ window }) => ((window as unknown as { __swiperEvents?: Reported[] }).__swiperEvents ?? []));
}

/** Opens the swiper demo on three image pages and starts recording its events. */
async function openSwiper(t: Fixture, id: string) {
  const { app, namespace, defer } = bindFixture(t, id);
  const shownIndex = () => app.logic.eval(({ pageStack }): number => {
    const page = pageStack().find((candidate) => candidate.route.includes('/swiper/'));
    return Number((page?.data as { index?: unknown } | undefined)?.index ?? -1);
  });

  const current = await currentPageOrNull(app);
  if (current?.name !== 'home') await app.nav.reset({ page: 'home' });
  await waitForCurrentPageVisible(app, 'home', '[data-testid="home-page"]');
  defer(async () => {
    const active = await currentPageOrNull(app);
    if (active?.name !== 'home') await app.nav.reset({ page: 'home' });
    await waitForCurrentPageVisible(app, 'home', '[data-testid="home-page"]');
  });

  await app.nav.go({ page: 'swiper' });
  await waitForCurrentPage(app, 'swiper');
  const view = (await app.page({ name: 'swiper' }, { timeout: 30_000 })).view;

  // The demo fills its swiper from the OS picker; the spec hands it files of its own instead.
  await app.logic.eval(async ({ lx, pageStack }, folder, pages) => {
    const items = [];
    for (const [name, base64] of pages) {
      const path = `${lx.fs.userData}/${folder}/${name}.png`;
      await lx.fs.write(path, base64, { encoding: 'base64', overwrite: true });
      items.push({ id: name, type: 'image' as const, src: path });
    }
    const page = pageStack().find((candidate) => candidate.route.includes('/swiper/'));
    page?.setData({ items, index: 0, eventLog: 'seeded' });
    return items.length;
  }, namespace, PAGES.map(([name, base64]) => [name, base64] as [string, string]));

  await view.css(SWIPER).first().waitFor({ state: 'visible', timeout: 30_000 });
  await view.eval(({ document, window }, selector) => {
    const log: unknown[] = [];
    (window as unknown as { __swiperEvents: unknown[] }).__swiperEvents = log;
    const swiper = document.querySelector(selector) as unknown as {
      addEventListener(type: string, listener: (event: { detail?: Record<string, unknown> }) => void): void;
    };
    for (const type of ['change', 'transitionend', 'endreached', 'tap', 'videoended', 'error']) {
      swiper.addEventListener(type, (event) => {
        const detail = event.detail ?? {};
        log.push({
          type,
          index: typeof detail.index === 'number' ? detail.index : null,
          previousIndex: typeof detail.previousIndex === 'number' ? detail.previousIndex : null,
          source: typeof detail.source === 'string' ? detail.source : null,
          item: ((detail.item ?? null) as { id?: string } | null)?.id ?? null,
        });
      });
    }
  }, SWIPER);
  expect(await shownIndex()).toBe(0);
  const next = () => view.eval(({ document }, selector) => {
    (document.querySelector(selector) as unknown as { next(): void }).next();
  }, SWIPER);
  return { app, view, shownIndex, next };
}

test('page through a native media swiper by command and by the index the page sets', {
  id: 'MEDIA-SWIPER-001',
  covers: ['NavDriver.to'],
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
}, async (t) => {
  const { app, view, shownIndex, next } = await openSwiper(t, 'MEDIA-SWIPER-001');

  await t.step('next() shows the second page and says which one', async () => {
    await next();
    await eventually(shownIndex, (index) => index === 1, { describe: 'the page index to become 1 after next()' });
    const change = (await reported(view)).find((event) => event.type === 'change');
    expect(change).toEqual({ type: 'change', index: 1, previousIndex: 0, source: 'api', item: 'green' });
  });

  await t.step('next() again shows the third page', async () => {
    await next();
    await eventually(shownIndex, (index) => index === 2, { describe: 'the page index to become 2 after next()' });
    const change = (await reported(view)).filter((event) => event.type === 'change').pop();
    expect(change).toEqual({ type: 'change', index: 2, previousIndex: 1, source: 'api', item: 'blue' });
  });

  await t.step('next() on the last page reports the end and stays there', async () => {
    await next();
    const events = await eventually(() => reported(view), (all) => all.some((event) => event.type === 'endreached'), {
      describe: 'endreached after next() on the last page',
      render: (all) => JSON.stringify(all),
    });
    const end = events.find((event) => event.type === 'endreached');
    expect(end).toEqual({ type: 'endreached', index: 2, previousIndex: null, source: 'api', item: 'blue' });
    expect(await shownIndex()).toBe(2);
  });

  await t.step('the index the page sets brings back the first page', async () => {
    const before = (await reported(view)).length;
    await app.logic.eval(({ pageStack }) => {
      pageStack().find((candidate) => candidate.route.includes('/swiper/'))?.setData({ index: 0 });
    });
    // The page asked for it, so nothing reports it back; the next command starts from it.
    const moved = await eventually(async () => {
      await next();
      await new Promise<void>((resolve) => setTimeout(() => resolve(), 400));
      return (await reported(view)).slice(before);
    }, (events) => events.some((event) => event.type === 'change'), {
      describe: 'a change after next() from the index the page set',
      intervalMs: 200,
      render: (events) => JSON.stringify(events),
    });
    expect(moved.find((event) => event.type === 'change')).toEqual({ type: 'change', index: 1, previousIndex: 0, source: 'api', item: 'green' });
    expect((await reported(view)).some((event) => event.type === 'error')).toBe(false);
  });

});

test('a finger swipe pages a native media swiper and settles on the next page', {
  id: 'MEDIA-SWIPER-002',
  covers: ['NavDriver.to'],
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
  requires: { input: ['touch'] },
}, async (t) => {
  const { view, shownIndex } = await openSwiper(t, 'MEDIA-SWIPER-002');
  const box = await view.eval(({ document }, selector) => {
    const rect = document.querySelector(selector)!.getBoundingClientRect();
    return { left: rect.left, top: rect.top, width: rect.width, height: rect.height };
  }, SWIPER);
  const y = box.top + box.height / 2;
  await view.swipe({ from: [box.left + box.width * 0.85, y], to: [box.left + box.width * 0.15, y], durationMs: 250, hold: 0 });
  await eventually(shownIndex, (index) => index === 1, { describe: 'the page index to become 1 after the swipe' });
  const events = await eventually(() => reported(view), (all) => all.some((event) => event.type === 'transitionend'), {
    describe: 'the swiper to settle after the swipe',
    render: (all) => JSON.stringify(all),
  });
  expect(events).toEqual([
    { type: 'change', index: 1, previousIndex: 0, source: 'touch', item: 'green' },
    { type: 'transitionend', index: 1, previousIndex: 0, source: 'touch', item: 'green' },
  ]);
});
