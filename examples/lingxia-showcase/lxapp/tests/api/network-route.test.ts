import { expect, spec } from '@lingxia/test';
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
    { status: 501, json: { error: 'unsupported_by_firmware' } },
  );
  await app.network.route(`${BASE}/clients`, { abort: 'failed' });

  const result = await app.eval({
    script: `
      const patched = await fetch(${JSON.stringify(`${BASE}/devices/d1`)}, {
        method: 'PATCH',
        body: JSON.stringify({ name: 'Office' }),
      });
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
      };
    `,
  }) as {
    status: number;
    contentType: string | null;
    body: { error: string };
    failure: { name: string; message: string } | null;
  };

  expect(result.status).toBe(501);
  expect(result.contentType).toBe('application/json');
  expect(result.body.error).toBe('unsupported_by_firmware');
  expect(result.failure).toEqual({ name: 'TypeError', message: 'fetch failed' });

  const patched = await patch.requests();
  expect(patched.map((entry) => [entry.method, entry.action, entry.status])).toEqual([['PATCH', 'fulfill', 501]]);
  const all = await app.network.requests();
  expect(all.map((entry) => entry.action)).toEqual(['fulfill', 'abort']);

  // `times: 1` already retired the PATCH route; only the abort route is left.
  expect(await patch.unroute()).toBe(false);
  expect(await app.network.unrouteAll()).toBe(1);
});

spec("reject network routes from inside app Logic", {
  id: "AUT-NET-002",
  covers: ['NetworkDriver.route'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, "AUT-NET-002");

  const rejection = await app.eval({
    script: `
      try {
        await lx.automation().lxapp().network.route('**', { status: 200 });
        return { rejected: false };
      } catch (error) {
        return { rejected: true, code: String(error?.code || ''), message: String(error?.message || error) };
      }
    `,
  }) as { rejected: boolean; code?: string; message?: string };

  expect(rejection.rejected).toBeTruthy();
  expect(rejection.code).toBe('E_AUTOMATION');
  expect(rejection.message).toContain('host automation run');
});
