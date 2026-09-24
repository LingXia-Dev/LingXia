import { expect, spec } from '@lingxia/test';
import type { NetworkRouteHandler } from '@lingxia/types/automation';
import { bindFixture } from '../helpers/poll.js';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import outage from '../fixtures/network/outage.json';
import statusOffline from '../scenarios/route/status-offline.json';

// Public host: the Showcase keeps the default public-network grant, and a
// route never fulfills a host the app's policy would refuse.
const BASE = 'https://api.example.com/lingxia-showcase/route';

spec("route Logic fetch to a faked error and a transport failure", {
  id: "AUT-NET-001",
  covers: ['NetworkDriver.route', 'NetworkDriver.requests', 'NetworkDriver.unrouteAll', 'NetworkRoute.requests', 'NetworkRoute.unroute'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, "AUT-NET-001");

  const patch = await app.network.route(
    { url: `${BASE}/devices/*`, method: 'PATCH', times: 1 },
    { status: 501, json: { error: 'not_implemented' } },
  );
  await app.network.route(`${BASE}/clients`, { abort: 'failed' });
  await app.network.route(`${BASE}/icon`, { body: new Uint8Array([137, 80, 78, 71]), contentType: 'image/png' });
  await app.network.route(`${BASE}/slow`, { status: 202, delay: 200 });

  const result = await app.eval({
    script: `
      const patched = await fetch(${JSON.stringify(`${BASE}/devices/d1`)}, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json', 'X-Request-Id': 'r-1' },
        body: JSON.stringify({ name: 'Office' }),
      });
      const icon = await fetch(${JSON.stringify(`${BASE}/icon`)});
      const iconBytes = Array.from(new Uint8Array(await icon.arrayBuffer()));
      const slowStarted = Date.now();
      const slow = await fetch(${JSON.stringify(`${BASE}/slow`)});
      const slowElapsed = Date.now() - slowStarted;
      let failure = null;
      try {
        await fetch(${JSON.stringify(`${BASE}/clients`)});
      } catch (error) {
        failure = { name: error.name, message: error.message };
      }
      return {
        status: patched.status,
        contentType: patched.headers.get('content-type'),
        body: await patched.json(),
        failure,
        iconBytes,
        slowStatus: slow.status,
        slowElapsed,
      };
    `,
  }) as {
    status: number;
    contentType: string | null;
    body: { error: string };
    failure: { name: string; message: string } | null;
    iconBytes: number[];
    slowStatus: number;
    slowElapsed: number;
  };

  expect(result.status).toBe(501);
  expect(result.contentType).toBe('application/json');
  expect(result.body.error).toBe('not_implemented');
  expect(result.failure).toEqual({ name: 'TypeError', message: 'fetch failed' });
  expect(result.iconBytes).toEqual([137, 80, 78, 71]);
  expect(result.slowStatus).toBe(202);
  expect(result.slowElapsed >= 150).toBeTruthy();

  const patched = await patch.requests();
  expect(patched.map((entry) => [entry.method, entry.action, entry.status])).toEqual([['PATCH', 'fulfill', 501]]);
  expect(patched[0].headers['x-request-id']).toBe('r-1');
  expect(JSON.parse(patched[0].body ?? 'null')).toEqual({ name: 'Office' });
  expect(patched[0].bodyTruncated).toBe(false);
  const all = await app.network.requests();
  // In the order the app sent them: PATCH, icon, slow, then the aborted read.
  expect(all.map((entry) => entry.action)).toEqual(['fulfill', 'fulfill', 'fulfill', 'abort']);

  // `times: 1` already retired the PATCH route; three routes are left.
  expect(await patch.unroute()).toBe(false);
  expect(await app.network.unrouteAll()).toBe(3);
});

spec("reject a route handler that mixes fulfill and abort", {
  id: "AUT-NET-003",
  covers: ['NetworkDriver.route'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, "AUT-NET-003");

  // The typings already refuse this shape; the host refuses it at runtime too.
  const mixed = { status: 500, abort: 'failed' } as unknown as NetworkRouteHandler;
  let message = '';
  try {
    await app.network.route(`${BASE}/mixed`, mixed);
  } catch (error) {
    message = String((error as Error)?.message ?? error);
  }
  expect(message).toContain('choose one of fulfill, abort, continue, or hang');
});

spec("reject network routes from inside app Logic", {
  id: "AUT-NET-002",
  covers: ['NetworkDriver.route'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, "AUT-NET-002");

  const rejection = await app.eval({
    script: `
      // Reading the driver works; only calls reject outside a host run.
      const network = lx.automation().lxapp().network;
      try {
        await network.route('**', { status: 200 });
        return { readable: typeof network.route === 'function', rejected: false };
      } catch (error) {
        return {
          readable: typeof network.route === 'function',
          rejected: true,
          code: String(error?.code || ''),
          message: String(error?.message || error),
        };
      }
    `,
  }) as { readable: boolean; rejected: boolean; code?: string; message?: string };

  expect(rejection.readable).toBe(true);
  expect(rejection.rejected).toBeTruthy();
  expect(rejection.code).toBe('E_AUTOMATION');
  expect(rejection.message).toContain('host automation run');
});

spec("serve a scenario file with a sequence, relative times and file-order precedence", {
  id: "AUT-NET-004",
  covers: [
    'NetworkDriver.scenario',
    'NetworkScenario.name',
    'NetworkScenario.routes',
    'NetworkScenario.requests',
    'NetworkScenario.unroute',
  ],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, "AUT-NET-004");

  const scenario = await app.network.scenario(outage);
  expect(scenario.name).toBe('showcase-outage');
  expect(scenario.routes.map((route) => route.pattern)).toEqual(outage.routes.map((route) => route.url));

  const result = await app.eval({
    script: `
      const base = ${JSON.stringify(BASE)};
      const statuses = [];
      let recovered = null;
      for (let i = 0; i < 4; i += 1) {
        const response = await fetch(base + '/status');
        statuses.push(response.status);
        if (response.ok) recovered = await response.json();
      }
      const device = await (await fetch(base + '/devices/d1')).json();
      const special = (await fetch(base + '/devices/special')).status;
      return { statuses, recovered, device, special, now: Date.now() };
    `,
  }) as {
    statuses: number[];
    recovered: { up: boolean; checkedAt: string } | null;
    device: { id: string; lastSeen: string; lastSeenMs: string; expires: string };
    special: number;
    now: number;
  };

  // Answers follow call order; the last one repeats.
  expect(result.statuses).toEqual([503, 502, 200, 200]);
  expect(result.recovered?.up).toBe(true);
  const near = (iso: string, offsetMs: number) => Math.abs(Date.parse(iso) - (result.now + offsetMs)) < 120_000;
  expect(near(result.recovered?.checkedAt ?? '', 0)).toBeTruthy();
  expect(near(result.device.lastSeen, -2 * 3600_000)).toBeTruthy();
  expect(near(result.device.expires, 30 * 60_000)).toBeTruthy();
  expect(Math.abs(Number(result.device.lastSeenMs) - (result.now - 2 * 3600_000)) < 120_000).toBeTruthy();
  expect(result.special).toBe(418);

  const [status] = scenario.routes;
  expect((await status.requests()).map((entry) => entry.status)).toEqual([503, 502, 200, 200]);
  expect((await scenario.requests()).length).toBe(6);
  expect(await scenario.unroute()).toBe(3);
});

spec("reject a scenario with an unknown field", {
  id: "AUT-NET-006",
  covers: ['NetworkDriver.scenario'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, "AUT-NET-006");
  await t.reject(
    () => app.network.scenario({ routes: [{ url: `${BASE}/x`, stauts: 200 }] }),
    { message: "routes[0]: unknown route handler option 'stauts'" },
  );
  await t.reject(
    () => app.network.scenario({ http: { routes: [{ url: `${BASE}/x`, status: 200 }] }, worker: {} }),
    { message: 'worker section of a scenario is not supported yet' },
  );
});

spec("serve a sectioned scenario file, the form `lxdev scenario use` takes", {
  id: "AUT-NET-007",
  covers: ['NetworkDriver.scenario', 'NetworkScenario.name', 'NetworkScenario.unroute'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, "AUT-NET-007");

  const scenario = await app.network.scenario(statusOffline);
  expect(scenario.name).toBe('Status offline');
  expect(scenario.routes.map((route) => route.pattern)).toEqual(statusOffline.http.routes.map((route) => route.url));

  const result = await app.eval({
    script: `
      const base = ${JSON.stringify(BASE)};
      const status = await fetch(base + '/status');
      const device = await (await fetch(base + '/devices/d1')).json();
      return { status: status.status, up: (await status.json()).up, online: device.online, lastSeen: device.lastSeen, now: Date.now() };
    `,
  }) as { status: number; up: boolean; online: boolean; lastSeen: string; now: number };

  expect(result.status).toBe(503);
  expect(result.up).toBe(false);
  expect(result.online).toBe(false);
  expect(Math.abs(Date.parse(result.lastSeen) - (result.now - 6 * 3600_000)) < 120_000).toBeTruthy();
  expect(await scenario.unroute()).toBe(2);
});

spec("stream SSE answers to fetch and to Rong.SSE, which reconnects with Last-Event-ID", {
  id: "AUT-NET-005",
  covers: ['NetworkDriver.route', 'NetworkRoute.requests'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, "AUT-NET-005");

  await app.network.route(`${BASE}/feed`, {
    sse: [
      { event: 'ready', data: { n: 1 }, id: '1' },
      { comment: 'keepalive' },
      { delayMs: 150 },
      { data: 'bye' },
      { drop: true },
    ],
  });
  const live = await app.network.route(`${BASE}/live`, {
    sequence: [
      // Dropped after two events: Rong.SSE reconnects with Last-Event-ID.
      { sse: [{ data: 'one', id: 'e1' }, { data: { two: 2 }, id: 'e2', retry: 20 }, { drop: true }] },
      // No drop: stays open like a live server until the spec ends.
      { sse: [{ event: 'late', data: 'three', id: 'e3' }] },
    ],
  });

  const result = await app.eval({
    script: `
      const base = ${JSON.stringify(BASE)};
      const started = Date.now();
      const feed = await fetch(base + '/feed');
      const feedType = feed.headers.get('content-type');
      const feedText = await feed.text();
      const feedMs = Date.now() - started;

      const sse = new Rong.SSE(base + '/live', { reconnect: { baseDelayMs: 10, maxDelayMs: 100 } });
      const events = [];
      for await (const event of sse) {
        events.push([event.type, event.data, event.id]);
        if (events.length === 3) break;
      }
      return { feedType, feedText, feedMs, events };
    `,
  }) as { feedType: string | null; feedText: string; feedMs: number; events: string[][] };

  expect(result.feedType).toBe('text/event-stream');
  expect(result.feedText).toBe('event: ready\nid: 1\ndata: {"n":1}\n\n: keepalive\ndata: bye\n\n');
  expect(result.feedMs >= 120).toBeTruthy();
  expect(result.events).toEqual([
    ['message', 'one', 'e1'],
    ['message', '{"two":2}', 'e2'],
    ['late', 'three', 'e3'],
  ]);

  const connections = await live.requests();
  expect(connections.length).toBe(2);
  expect(connections[0].headers['accept']).toBe('text/event-stream');
  expect(connections[0].headers['last-event-id']).toBe(undefined);
  expect(connections[1].headers['last-event-id']).toBe('e2');
});
