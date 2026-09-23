import { expect, spec } from '@lingxia/test';
import type { NetworkRouteHandler } from '@lingxia/types/automation';
import { bindFixture } from '../helpers/poll.js';
import { SHOWCASE_APP_ID } from '../helpers/app.js';

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
  expect(message).toContain('choose one of fulfill, abort, or continue');
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
