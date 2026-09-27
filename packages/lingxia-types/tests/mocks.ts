import type { Mocks as RootMocks } from '../src/index.js';
import type { MockAnswer, MockHandler, MockRequest, Mocks } from '../src/mocks.js';

let signedIn = true;
const mocks = {
  'POST **/sessions': async (req: MockRequest) => {
    const { email } = await req.json<{ email: string }>();
    if (!email.includes('@')) return { status: 401, json: { reason: 'Check your email.' }, delay: 600 };
    signedIn = true;
    return { json: { id: 'me' } };
  },
  'DELETE **/sessions/current': () => {
    signedIn = false;
    return { status: 204 };
  },
  'GET **/devices': () => (signedIn ? { json: [] } : { status: 401 }),
  'GET **/legal': { json: [] },
  'GET **/events': { sse: [{ data: { state: 'done' } }, { drop: true }] },
  'GET **/qoe': { continue: true },
  'GET **/patched': { continue: true, patchJson: { total: 0 } },
  'GET **/proxy': async (_req, ctx) => ({ json: await (await ctx.fetch('https://h/x')).json() }),
} satisfies Mocks;
void mocks;
const root: RootMocks = mocks;
void root;

// A handler returns one answer per call: a sequence is a scenario field.
// @ts-expect-error sequence is not a mock answer
const sequence: MockAnswer = { sequence: [{ status: 200 }] };
// @ts-expect-error json and body are exclusive
const both: MockAnswer = { json: 1, body: 'x' };
void [sequence, both];
const handler: MockHandler = (req) => ({ json: { method: req.method, path: req.url.pathname } });
void handler;
