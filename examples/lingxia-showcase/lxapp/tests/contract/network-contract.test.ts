import { expect, spec, type Fixture } from '@lingxia/test';
import { SHOWCASE_APP_ID, rawApp } from '../helpers/app.js';

// String scripts and raw page reads go to the raw driver; see `rawApp`.
const raw = rawApp();

// Contract checks for routed Logic fetch, run with their document:
//
//   lxdev test tests/contract --openapi tests/contract/devices.openapi.yaml \
//     --covers-manifest tests/contract/coverage.yaml
//
// Every response a route fulfills is validated against the document; one
// that breaks it fails its spec with E_OPENAPI_CONTRACT. Without --openapi
// (`lxdev test tests/`, a platform entry) they skip themselves.
spec.configure({ tags: ['routed', 'contract'] });

spec.beforeEach((t) => {
  if (!t.openapi) {
    t.skip('needs its OpenAPI document: run with --openapi tests/contract/devices.openapi.yaml');
  }
});

const BASE = 'https://api.example.com/lingxia-showcase/contract/v1';

async function fetchJson(t: Fixture, url: string, method = 'GET') {
  return await raw.eval<{ status: number; body: unknown }>({
    script: `
      const response = await fetch(${JSON.stringify(url)}, { method: ${JSON.stringify(method)} });
      return { status: response.status, body: await response.json() };
    `,
  });
}

spec('a routed device list follows the contract', {
  id: 'CONTRACT-001',
  covers: ['CONTRACT-LIST'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  await t.app.network.route(`${BASE}/devices`, {
    json: { items: [{ id: 'd1', name: 'Office', room: null, kind: 'router' }], total: 1 },
  });
  const { status, body } = await fetchJson(t, `${BASE}/devices?page=1`);
  expect(status).toBe(200);
  expect(body).toMatchSchema('DeviceList');
  expect((body as { items: unknown[] }).items[0]).toMatchSchema('Device');
});

spec('a routed client error carries a problem body', {
  id: 'CONTRACT-002',
  covers: ['CONTRACT-ERROR'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  await t.app.network.route(`${BASE}/devices`, {
    status: 403,
    json: { error: 'forbidden' },
    contentType: 'application/problem+json',
  });
  const { status, body } = await fetchJson(t, `${BASE}/devices`);
  expect(status).toBe(403);
  expect(body).toMatchSchema({ ref: '#/components/schemas/Problem' });
});

spec.fail('a fixture that drifted from the contract fails its spec', {
  id: 'CONTRACT-003',
  covers: ['CONTRACT-RENAME'],
  app: SHOWCASE_APP_ID,
  expected: { code: 'E_OPENAPI_CONTRACT' },
}, async (t) => {
  // `kind` is required and `name` must not be empty: the route is stale.
  await t.app.network.route({ url: `${BASE}/devices/*`, method: 'PATCH' }, { json: { id: 'd1', name: '' } });
  const { status } = await fetchJson(t, `${BASE}/devices/d1`, 'PATCH');
  expect(status).toBe(200);
});
