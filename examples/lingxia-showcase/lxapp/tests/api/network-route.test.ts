import { expect, spec } from '@lingxia/test';
import type { NetworkRouteHandler, ScenarioInput } from '@lingxia/types/automation';
import { bindFixture } from '../helpers/poll.js';
import { SHOWCASE_APP_ID, rawApp } from '../helpers/app.js';
import outage from '../fixtures/network/outage.json';
import status from '../scenarios/route/status.json';

// String scripts and raw page reads go to the raw driver; see `rawApp`.
const raw = rawApp();

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

  const result = await app.logic.eval(async (_scope, base) => {
    const patched = await fetch(`${base}/devices/d1`, {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json', 'X-Request-Id': 'r-1' },
      body: JSON.stringify({ name: 'Office' }),
    });
    const icon = await fetch(`${base}/icon`);
    const iconBytes = Array.from(new Uint8Array(await icon.arrayBuffer()));
    const slowStarted = Date.now();
    const slow = await fetch(`${base}/slow`);
    const slowElapsed = Date.now() - slowStarted;
    let failure: { name: string; message: string } | null = null;
    try {
      await fetch(`${base}/clients`);
    } catch (error) {
      failure = { name: (error as Error).name, message: (error as Error).message };
    }
    return {
      status: patched.status,
      contentType: patched.headers.get('content-type'),
      body: await patched.json() as { error: string },
      failure,
      iconBytes,
      slowStatus: slow.status,
      slowElapsed,
    };
  }, BASE);

  expect(result.status).toBe(501);
  expect(result.contentType).toBe('application/json');
  expect(result.body.error).toBe('not_implemented');
  expect(result.failure).toEqual({ name: 'TypeError', message: 'fetch failed' });
  expect(result.iconBytes).toEqual([137, 80, 78, 71]);
  expect(result.slowStatus).toBe(202);
  expect(result.slowElapsed >= 150).toBeTruthy();

  // The call the route answered, already made: waitForCall hands it out.
  const call = await patch.waitForCall();
  expect([call.method, call.url, call.answeredBy, call.status]).toEqual(['PATCH', `${BASE}/devices/d1`, 'route', 501]);
  expect(call.headers?.['x-request-id']).toBe('r-1');
  expect(call.body).toEqual({ name: 'Office' });
  expect(await patch.calls()).toEqual([call]);
  const all = await app.network.calls();
  // In the order the app sent them: PATCH, icon, slow, then the aborted read.
  expect(all.map((entry) => [entry.answeredBy, entry.status])).toEqual([
    ['route', 501], ['route', 200], ['route', 202], ['route', null],
  ]);

  // `times: 1` already retired the PATCH route; removing it again is no error.
  expect(await patch.unroute()).toBeUndefined();
  expect(await app.network.unrouteAll()).toBeUndefined();
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

  const rejection = await raw.eval({
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
    'LxAppDriver.scenario',
    'Scenario.name',
    'Scenario.variant',
    'Scenario.rules',
    'Scenario.calls',
    'Scenario.unroute',
  ],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, "AUT-NET-004");

  const scenario = await app.scenario(outage);
  expect(scenario.name).toBe('showcase-outage');
  expect(scenario.variant).toBe(null);
  expect(scenario.rules.map((rule) => rule.target)).toEqual(outage.rules.map((rule) => rule.http));

  const result = await raw.eval({
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

  expect(scenario.rules.map((rule) => rule.hits)).toEqual([4, 1, 1]);
  const statusCalls = await scenario.calls({ http: `GET ${BASE}/status` });
  expect(statusCalls.map((call) => call.status)).toEqual([503, 502, 200, 200]);
  expect(statusCalls.every((call) => call.answeredBy === 'rule' && call.rule === 1)).toBeTruthy();
  // Successive waits hand out successive calls to the target.
  const first = await scenario.waitForCall({ http: `GET ${BASE}/status` });
  const second = await scenario.waitForCall({ http: `GET ${BASE}/status` });
  expect([first.status, second.status]).toEqual([503, 502]);
  expect((await scenario.calls()).length).toBe(6);
  expect(await scenario.remove()).toBeUndefined();
});

spec("reject a scenario the host cannot install, naming the rule", {
  id: "AUT-NET-006",
  covers: ['LxAppDriver.scenario'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, "AUT-NET-006");
  await t.reject(
    () => app.scenario({ rules: [{ http: `GET ${BASE}/x`, stauts: 200 }] }),
    { message: "rules[0]: unknown route handler option 'stauts'" },
  );
  await t.reject(
    // The old `routes` form, as an older file would still have it.
    () => app.scenario({ routes: [{ url: `${BASE}/x`, status: 200 }] } as object as ScenarioInput),
    { message: "'routes' is the old scenario format" },
  );
  await t.reject(
    () => app.scenario(status, 'degraded'),
    { message: "no variant 'degraded' (variants: " },
  );
  // Function rules need a companion that answers them; this session has
  // none, so nothing of the scenario is installed.
  await t.reject(
    () => app.scenario({
      rules: [
        { http: `GET ${BASE}/status`, status: 200 },
        { function: 'orders.submit', fault: 'unknown' },
      ],
    }),
    { message: '1 function rule (rule 2 function orders.submit) cannot be installed' },
  );
});

spec("switch a scenario's variant mid-spec and answer renames by their JSON body", {
  id: "AUT-NET-007",
  covers: ['LxAppDriver.scenario', 'Scenario.variant', 'Scenario.calls', 'Scenario.rules'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, "AUT-NET-007");
  // Below the scenario's rules: a rename no rule matches lands here instead
  // of the real network.
  await app.network.route({ url: `${BASE}/devices/*`, method: 'PATCH' }, { status: 404, json: { error: 'unmatched' } });

  const readStatus = () => raw.eval({
    script: `
      const response = await fetch(${JSON.stringify(`${BASE}/status`)});
      return { status: response.status, up: (await response.json()).up };
    `,
  }) as Promise<{ status: number; up: boolean }>;

  const online = await app.scenario(status, 'online');
  expect(online.variant).toBe('online');
  expect(online.rules.map((rule) => rule.target)[0]).toBe(`GET ${BASE}/status`);
  expect(await readStatus()).toEqual({ status: 200, up: true });

  // The next call answers from the other variant.
  const offline = await app.scenario(status, 'offline');
  expect(await readStatus()).toEqual({ status: 503, up: false });

  const result = await raw.eval({
    script: `
      const base = ${JSON.stringify(BASE)};
      const rename = async (body) => {
        const response = await fetch(base + '/devices/d1', {
          method: 'PATCH',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(body),
        });
        return [response.status, await response.json()];
      };
      const device = await (await fetch(base + '/devices/d1')).json();
      return {
        device,
        taken: await rename({ name: 'Office', floor: 3 }),
        lab: await rename({ name: 'Lab-12', tags: ['lab', 'floor-2'] }),
        // One tag short: arrays match exactly.
        missed: await rename({ name: 'Lab-12', tags: ['lab'] }),
        now: Date.now(),
      };
    `,
  }) as {
    device: { online: boolean; lastSeen: string };
    taken: [number, { error: string }];
    lab: [number, { renamed: boolean }];
    missed: [number, { error: string }];
    now: number;
  };
  expect(result.device.online).toBe(false);
  expect(Math.abs(Date.parse(result.device.lastSeen) - (result.now - 6 * 3600_000)) < 120_000).toBeTruthy();
  expect(result.taken).toEqual([409, { error: 'name_taken' }]);
  expect(result.lab).toEqual([200, { renamed: true }]);
  expect(result.missed).toEqual([404, { error: 'unmatched' }]);

  const renames = await offline.calls({ http: `PATCH ${BASE}/devices/*` });
  expect(renames.map((call) => [call.answeredBy, call.rule])).toEqual([['rule', 3], ['rule', 4], ['route', undefined]]);
  expect(renames[0].body).toEqual({ name: 'Office', floor: 3 });
  expect(renames[2].noMatch ?? '').toContain('rule 4 match.json.tags: expected an array of 2, got 1');
  // Variant rules come first: rule 1 is offline's status rule.
  expect(offline.rules.map((rule) => [rule.index, rule.hits])).toEqual([[1, 1], [2, 1], [3, 1], [4, 1]]);
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

  const result = await raw.eval({
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

  const connections = await live.calls();
  expect(connections.length).toBe(2);
  expect(connections[0].headers?.['accept']).toBe('text/event-stream');
  expect(connections[0].headers?.['last-event-id']).toBe(undefined);
  expect(connections[1].headers?.['last-event-id']).toBe('e2');
});
