import { expect, spec, type TestView } from '@lingxia/test';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { currentPageOrNull, waitForCurrentPage, waitForCurrentPageVisible } from '../helpers/page.js';
import { bindFixture, eventually } from '../helpers/poll.js';

const httpBase = (globalThis.__LINGXIA_AUTOMATION_HOST__?.args ?? {}).httpBase;
spec.configure({ skip: httpBase ? undefined : 'needs --arg httpBase' });
const SWIPER = '#lx-media-swiper-demo';
// A 64x40 solid red PNG: the page before the clip.
const RED = 'iVBORw0KGgoAAAANSUhEUgAAAEAAAAAoCAIAAADBrGu+AAAAPUlEQVR42u3PQQkAAAgEsIt2/QOYxww+hcEKLNO+FgEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQGBiwX4qFzEMNmn2gAAAABJRU5ErkJggg==';

interface Reported {
  type: string;
  index: number | null;
  item: string | null;
}

function reported(view: TestView): Promise<Reported[]> {
  return view.eval(({ window }) => ((window as unknown as { __swiperEvents?: Reported[] }).__swiperEvents ?? []));
}

spec('play a video page of a native media swiper to its end', {
  id: 'MEDIA-SWIPER-003',
  covers: ['NavDriver.to'],
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
  reason: 'needs the HTTP fixture: node tests/harness/http-fixture.mjs, then --arg httpBase=<url>',
}, async (t) => {
  const { app, namespace, defer } = bindFixture(t, 'MEDIA-SWIPER-003');

  const current = await currentPageOrNull(app);
  if (current?.name !== 'home') await app.nav.relaunch({ page: 'home' });
  await waitForCurrentPageVisible(app, 'home', '[data-testid="home-page"]');
  defer(async () => {
    const active = await currentPageOrNull(app);
    if (active?.name !== 'home') await app.nav.relaunch({ page: 'home' });
    await waitForCurrentPageVisible(app, 'home', '[data-testid="home-page"]');
  });

  await app.nav.to({ page: 'swiper' });
  await waitForCurrentPage(app, 'swiper');
  const view = (await app.page({ name: 'swiper' }, { timeout: 30_000 })).view;

  await app.logic.eval(async ({ lx, getCurrentPages }, folder, red, clip) => {
    const path = `${lx.env.USER_DATA_PATH}/${folder}/red.png`;
    await lx.fs.write(path, red, { encoding: 'base64', overwrite: true });
    const page = getCurrentPages().find((candidate) => candidate.route.includes('/swiper/'));
    page?.setData({
      items: [
        { id: 'red', type: 'image' as const, src: path },
        { id: 'clip', type: 'video' as const, src: clip },
      ],
      index: 0,
      eventLog: 'seeded',
    });
  }, namespace, RED, `${httpBase}/media/sample.mp4`);

  await view.css(SWIPER).first().waitFor({ state: 'visible', timeout: 30_000 });
  await view.eval(({ document, window }, selector) => {
    const log: unknown[] = [];
    (window as unknown as { __swiperEvents: unknown[] }).__swiperEvents = log;
    const swiper = document.querySelector(selector) as unknown as {
      addEventListener(type: string, listener: (event: { detail?: Record<string, unknown> }) => void): void;
    };
    for (const type of ['change', 'videoended', 'error']) {
      swiper.addEventListener(type, (event) => {
        const detail = event.detail ?? {};
        log.push({
          type,
          index: typeof detail.index === 'number' ? detail.index : null,
          item: ((detail.item ?? null) as { id?: string } | null)?.id ?? null,
        });
      });
    }
  }, SWIPER);

  // The clip is four seconds long and starts when its page is shown.
  await view.eval(({ document }, selector) => {
    (document.querySelector(selector) as unknown as { next(): void }).next();
  }, SWIPER);
  const events = await eventually(() => reported(view), (all) => all.some((event) => event.type === 'videoended' || event.type === 'error'), {
    describe: 'the clip to play to its end',
    timeoutMs: 30_000,
    intervalMs: 200,
    render: (all) => JSON.stringify(all),
  });
  expect(events.filter((event) => event.type !== 'change')).toEqual([{ type: 'videoended', index: 1, item: 'clip' }]);
});
