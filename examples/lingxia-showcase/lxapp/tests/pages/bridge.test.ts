import { expect, spec } from '@lingxia/test';
import type { LxAppDriver } from '@lingxia/types/automation';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { waitForCurrentPage, waitForElementText } from '../helpers/page.js';
import { bindFixture, eventually } from '../helpers/poll.js';

// Document port handshake across the ways a page document comes to exist:
// a hidden preloaded tab, a pushed page, a covered page coming back, a
// re-entered route, rapid re-entry, and a restarted app. `bridge-repro` is the
// strict probe: its Bootstrap verdict needs bridge ready + the initial Logic
// snapshot + a View→Logic round trip, and its stream audits Logic→View ticks
// for gaps.

const REPRO = 'bridge-repro';
// Tab pages from lxapp.json, in bar order, after home.
const TABS = ['api', 'components', 'todo', 'media', 'device', 'surface'] as const;
const DOCUMENT_MARK = '__lxBridgeSpecDocument';

const anyTransition = () => true;

async function bridgeReady(app: LxAppDriver, page: string): Promise<boolean> {
  return eventually(
    async () => (await app.page.eval({
      page,
      script: 'window.LingXiaBridge?.isReady?.() === true',
    })) === true,
    (ready) => ready,
    { timeoutMs: 15_000, describe: `${page} bridge handshake`, retryIf: anyTransition },
  );
}

async function expectBootstrap(app: LxAppDriver): Promise<void> {
  await app.page.waitFor({
    page: REPRO,
    css: '[data-testid="bridge-repro-page"][data-automation-contract="bridge-v1"]',
    timeoutMs: 20_000,
  });
  // The page reports FAIL once its 5 s deadline passes and flips to PASS if the
  // handshake completes later, so only PASS ends the wait.
  expect(await waitForElementText(app, REPRO, '#bootstrap-verdict', (text) => text.includes('PASS'), 20_000))
    .toContain('PASS');
}

async function expectEcho(app: LxAppDriver, n: number): Promise<void> {
  await app.page.click({ page: REPRO, css: '#btn-echo' });
  expect(await waitForElementText(app, REPRO, '#stat-echo', (text) => text.includes(`echo #${n} `), 10_000))
    .toContain(`echo #${n} ok`);
}

async function expectGapFreeStream(app: LxAppDriver): Promise<void> {
  await app.page.click({ page: REPRO, css: '#btn-restart' });
  await waitForElementText(
    app,
    REPRO,
    '#stat-received',
    (text) => Number.parseInt(text.replace(/\D+/g, ''), 10) >= 20,
    20_000,
  );
  expect(await waitForElementText(app, REPRO, '#stream-verdict', (text) => /PASS|FAIL/.test(text)))
    .toContain('PASS');
  expect(await waitForElementText(app, REPRO, '#stat-gaps', () => true)).toContain('none');
  await app.page.click({ page: REPRO, css: '#btn-stop' });
}

async function markDocument(app: LxAppDriver, page: string): Promise<void> {
  await app.page.eval({ page, script: `(window.${DOCUMENT_MARK} = true, true)` });
}

async function isMarkedDocument(app: LxAppDriver, page: string): Promise<boolean> {
  return (await app.page.eval({ page, script: `window.${DOCUMENT_MARK} === true` })) === true;
}

async function enterRepro(app: LxAppDriver): Promise<void> {
  await app.nav.to({ page: REPRO });
  await waitForCurrentPage(app, REPRO, 20_000);
}

async function startAtHome(app: LxAppDriver): Promise<void> {
  await app.nav.relaunch({ page: 'home' });
  await waitForCurrentPage(app, 'home', 20_000);
}

spec('hand every preloaded tab a working port once it is shown', {
  id: 'BRIDGE-001',
  covers: ['lx.switchTab'],
  app: SHOWCASE_APP_ID,
  timeout: 120_000,
}, async (t) => {
  const { app, defer } = bindFixture(t, 'BRIDGE-001');
  defer(async () => {
    await app.nav.relaunch({ page: 'home' });
  });

  await startAtHome(app);
  // Physical stabilization: let the preloaded tab documents finish loading
  // while hidden. That is the state in which a platform may never deliver a
  // visible-commit signal, so switching sooner would not exercise it.
  await new Promise<void>((resolve) => setTimeout(resolve, 2_000));

  for (const tab of TABS) {
    await app.nav.switchTab({ page: tab });
    await waitForCurrentPage(app, tab, 20_000);
    expect(await bridgeReady(app, tab)).toBe(true);
  }
  await app.nav.switchTab({ page: 'home' });
  await waitForCurrentPage(app, 'home', 20_000);
  expect(await bridgeReady(app, 'home')).toBe(true);
});

spec('bootstrap a pushed page through its own port', {
  id: 'BRIDGE-002',
  covers: ['lx.navigateTo'],
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
}, async (t) => {
  const { app, defer } = bindFixture(t, 'BRIDGE-002');
  defer(async () => {
    await app.nav.relaunch({ page: 'home' });
  });

  await startAtHome(app);
  await enterRepro(app);
  await expectBootstrap(app);
  await expectEcho(app, 1);
  await expectGapFreeStream(app);
});

spec("keep a covered page's port across navigateTo and back", {
  id: 'BRIDGE-003',
  covers: ['lx.navigateTo', 'lx.navigateBack'],
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
}, async (t) => {
  const { app, defer } = bindFixture(t, 'BRIDGE-003');
  defer(async () => {
    await app.nav.relaunch({ page: 'home' });
  });

  await startAtHome(app);
  await enterRepro(app);
  await expectBootstrap(app);
  await expectEcho(app, 1);

  await app.nav.to({ page: 'device', query: { type: 'screen' } });
  await waitForCurrentPage(app, 'device', 20_000);
  await app.nav.back();
  await waitForCurrentPage(app, REPRO, 20_000);

  // The same document and page instance: its echo counter continues.
  expect(await bridgeReady(app, REPRO)).toBe(true);
  await expectEcho(app, 2);
});

spec('bootstrap a re-entered route as a fresh document', {
  id: 'BRIDGE-004',
  covers: ['lx.navigateTo', 'lx.navigateBack'],
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
}, async (t) => {
  const { app, defer } = bindFixture(t, 'BRIDGE-004');
  defer(async () => {
    await app.nav.relaunch({ page: 'home' });
  });

  await startAtHome(app);
  await enterRepro(app);
  await expectBootstrap(app);
  await markDocument(app, REPRO);

  await app.nav.back();
  await waitForCurrentPage(app, 'home', 20_000);
  await enterRepro(app);

  // Same URL, new document: a port bound to the previous one must not serve it.
  await eventually(
    () => isMarkedDocument(app, REPRO),
    (marked) => !marked,
    { timeoutMs: 15_000, describe: 're-entered bridge-repro to be a new document', retryIf: anyTransition },
  );
  await expectBootstrap(app);
  await expectEcho(app, 1);
});

spec('settle on a working port after rapid re-entry', {
  id: 'BRIDGE-005',
  covers: ['lx.navigateTo', 'lx.navigateBack'],
  app: SHOWCASE_APP_ID,
  timeout: 90_000,
}, async (t) => {
  const { app, defer } = bindFixture(t, 'BRIDGE-005');
  defer(async () => {
    await app.nav.relaunch({ page: 'home' });
  });

  await startAtHome(app);
  // Leave each document before its handshake can finish, so late load and
  // port callbacks for a replaced document race the next one.
  for (let round = 0; round < 3; round += 1) {
    await app.nav.to({ page: REPRO });
    await app.nav.back();
  }
  await waitForCurrentPage(app, 'home', 20_000);

  await enterRepro(app);
  await expectBootstrap(app);
  await expectEcho(app, 1);
  await expectGapFreeStream(app);
});

spec('bring the bridge back up after the app restarts', {
  id: 'BRIDGE-006',
  covers: ['LxAppManager.restart'],
  app: SHOWCASE_APP_ID,
  timeout: 90_000,
}, async (t) => {
  const { app: before, defer } = bindFixture(t, 'BRIDGE-006');
  // Restart replaces the LxApp instance and relaunches the initial route. A
  // driver stays bound to the instance it was created for, so everything after
  // the restart addresses the new instance through a fresh driver.
  const freshDriver = () => lx.automation().lxapp(SHOWCASE_APP_ID);
  defer(async () => {
    await freshDriver().nav.relaunch({ page: 'home' });
  });

  await startAtHome(before);
  await enterRepro(before);
  await expectBootstrap(before);

  await lx.automation().lxapps.restart({ app: SHOWCASE_APP_ID });
  // `restart` resolves before the replacement instance exists, so resolve a
  // new driver on every poll until one reports the relaunched home.
  await eventually(
    () => freshDriver().nav.current(),
    (current) => current.name === 'home' && current.ready,
    { timeoutMs: 30_000, describe: 'restarted app to relaunch home', retryIf: anyTransition },
  );
  const app = freshDriver();
  expect(await bridgeReady(app, 'home')).toBe(true);

  await enterRepro(app);
  await expectBootstrap(app);
  await expectEcho(app, 1);
});

spec('re-enter a route the moment it bootstraps without stalling its port', {
  id: 'BRIDGE-007',
  covers: ['lx.navigateTo', 'lx.navigateBack'],
  app: SHOWCASE_APP_ID,
  timeout: 150_000,
}, async (t) => {
  const { app, defer } = bindFixture(t, 'BRIDGE-007');
  defer(async () => {
    await app.nav.relaunch({ page: 'home' });
  });

  await startAtHome(app);
  // Leaving parks the document and re-entering at once reloads over it. A port
  // request lost in that swap recovers only through the page's 5 s port
  // timeout, which lands in the probe latency measured from View mount. The
  // race is timing dependent, so it gets several rounds.
  for (let round = 0; round < 4; round += 1) {
    await enterRepro(app);
    await expectBootstrap(app);
    await markDocument(app, REPRO);
    await app.nav.back();
    await enterRepro(app);
    await eventually(
      () => isMarkedDocument(app, REPRO),
      (marked) => !marked,
      { timeoutMs: 15_000, describe: `round ${round}: re-entered bridge-repro to be a new document`, retryIf: anyTransition },
    );
    await expectBootstrap(app);
    const probe = await waitForElementText(app, REPRO, '#bootstrap-probe', (text) => /\(\d+ ms\)/.test(text), 20_000);
    const latency = Number.parseInt(probe.replace(/^[\s\S]*\((\d+) ms\)[\s\S]*$/, '$1'), 10);
    expect(latency <= 5_000 ? 'no port stall' : `round ${round}: port stalled ${latency} ms`).toBe('no port stall');
    await app.nav.back();
    await waitForCurrentPage(app, 'home', 20_000);
  }
});
