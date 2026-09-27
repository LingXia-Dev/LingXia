import { expect, spec } from '@lingxia/test';
import { bindFixture } from '../helpers/poll.js';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import profile from '../scenarios/mocks/profile.json';

// `mocks/config.json` sends these calls to `mocks/index.ts`: the app calls
// `fetch`, and a handler answers in development. Public host, like the
// route specs: nothing answers a host the app's policy would refuse.
const BASE = 'https://api.example.com/lingxia-showcase/mocks';

type Read = { status: number; body: Record<string, unknown> };

spec("answer the selected calls from mocks/ handlers, with a scenario state on top", {
  id: "AUT-NET-008",
  covers: ['LxAppDriver.mock', 'MockDriver.use'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, "AUT-NET-008");
  const read = () => app.logic.eval(async (_scope, base): Promise<Read> => {
    const response = await fetch(base + '/profile');
    return { status: response.status, body: await response.json() as Record<string, unknown> };
  }, BASE);

  // Each spec starts with fresh handler state; module state then lives
  // across calls.
  expect(await read()).toEqual({ status: 200, body: { id: 'u1', name: 'Ada', plan: 'free', visits: 1 } });
  const renamed = await app.logic.eval(async (_scope, base) => {
    const response = await fetch(base + '/profile', {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ name: 'Lin' }),
    });
    return await response.json() as { name: string };
  }, BASE);
  expect(renamed.name).toBe('Lin');
  expect((await read()).body).toEqual({ id: 'u1', name: 'Lin', plan: 'free', visits: 2 });

  // A scenario variant answers before the handlers...
  const suspended = await app.mock.use(profile, 'suspended');
  expect(suspended.variant).toBe('suspended');
  expect(await read()).toEqual({ status: 403, body: { error: 'suspended' } });
  // ...or lets the handler answer and patches it. A second `use` replaces
  // the first.
  const patched = await app.mock.use(profile, 'renamed');
  expect((await read()).body).toEqual({ id: 'u1', name: 'Grace', plan: 'free', visits: 3 });
  const calls = await patched.calls({ http: `GET ${BASE}/profile` });
  expect(calls.map((call) => [call.answeredBy, call.rule])).toEqual([['rule', 1]]);
  await patched.remove();
  expect((await read()).body.name).toBe('Lin');

  // A handler that throws fails the request; it never falls through to
  // the real backend.
  const broken = await app.logic.eval(async (_scope, base) => {
    try {
      await fetch(base + '/broken');
      return 'resolved';
    } catch (error) {
      const failure = error as Error & { data?: { detail?: string } };
      return `${failure.name}: ${failure.message} — ${failure.data?.detail ?? ''}`;
    }
  }, BASE);
  expect(broken).toBe(
    `TypeError: fetch failed — mock handler 'GET ${BASE}/broken' threw: the handler broke`,
  );
});

spec("start every spec with fresh mock handler state", {
  id: "AUT-NET-009",
  covers: ['MockDriver.reset'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, "AUT-NET-009");
  // Whatever an earlier spec did to the handlers' module state is gone.
  const first = await app.logic.eval(async (_scope, base) => {
    const response = await fetch(base + '/profile');
    return await response.json() as Record<string, unknown>;
  }, BASE);
  expect(first).toEqual({ id: 'u1', name: 'Ada', plan: 'free', visits: 1 });
});
